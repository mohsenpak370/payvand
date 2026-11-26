<script setup lang="ts">
import type { Product } from '~/composables/useShop'
import { priceInCurrency } from '~/composables/useShop'
import type { SupportedCurrency } from '~/composables/usePaymentApi'

// One shop's page: its name, its description, the currency its menu is priced
// in, and what it sells. Everything here is the seller's own menu — the shop's
// payment page reads the same catalogue and turns a cart of it into a receipt.
const props = defineProps<{ shopId: string }>()

const { t } = useI18n()
const api = useShopApi()

/** The shop's saved headings, and whether the suggestion list is showing. */
const categoryOptions = ref<string[]>([])
const categoryOpen = ref(false)
const categorySuggestions = computed(() => {
  const typed = productCategory.value.trim().toLowerCase()
  return categoryOptions.value
    .filter(name => !typed || (name.toLowerCase().includes(typed) && name.toLowerCase() !== typed))
    .slice(0, 6)
})

async function loadCategories(): Promise<void> {
  try {
    categoryOptions.value = await api.categories(props.shopId)
  } catch {
    // A list that cannot be read leaves the input a plain field, not a broken form.
    categoryOptions.value = []
  }
}

async function deleteCategory(name: string): Promise<void> {
  try {
    categoryOptions.value = await api.deleteCategory(props.shopId, name)
  } catch {
    // The list it came from is still what the shop has.
  }
}

function pickCategory(name: string): void {
  productCategory.value = name
  categoryOpen.value = false
}

const shopName = ref('')
const shopDescription = ref('')
const products = ref<Product[]>([])

const loading = ref(true)
const savingShop = ref(false)
/** Set when the shop itself could not be opened — the page shows nothing else. */
const loadFailure = ref('')
const failure = ref('')

const { currency } = useCurrencyPreference()
const rates = useSolRates()

/**
 * What the whole menu is priced — and charged — in. A stablecoin price is its
 * own money, not a conversion of SOL, so this is shop data saved with the shop,
 * and the shop's payment page charges in it.
 */
const shopCurrency = ref<SupportedCurrency>('sol')
/** The currency the saved prices are in, which is what a change converts from. */
const savedCurrency = ref<SupportedCurrency>('sol')

const currencyOptions: { label: string, value: SupportedCurrency }[] = [
  { label: 'SOL', value: 'sol' },
  { label: 'USDC', value: 'usdc' },
  { label: 'EURC', value: 'eurc' }
]

/** The unit as it is written after a price field. */
const unitLabel = computed(() => currencySymbol(shopCurrency.value))

/** A price typed in the shop's currency, in that currency's base units. */
const priceBaseUnits = computed(() => {
  const typed = Number(productPrice.value)
  if (!Number.isFinite(typed) || typed <= 0) return 0
  return shopCurrency.value === 'sol' ? solToLamports(typed) : Math.round(typed * 1_000_000)
})

const editingId = ref<string | null>(null)
const formOpen = ref(false)
const productName = ref('')
const productDetails = ref('')
const productCategory = ref('')
const productPrice = ref('')
const productBusy = ref(false)
const productFailure = ref('')

const canSubmitProduct = computed(() =>
  !productBusy.value
  && productName.value.trim().length > 0
  && priceBaseUnits.value > 0
)

/** A stored price as a number for the price field. */
function priceInputValue(amount: number): string {
  return shopCurrency.value === 'sol'
    ? lamportsToSol(amount)
    : formatTokenAmount(amount, 6)
}

/** A stored price in the reader's own money, when the rate is known. */
function priceHint(amount: number): string | null {
  const value = fiatValueOf(shopCurrency.value, amount, rates.value, currency.value)
  return value === null ? null : formatFiat(value, currency.value)
}

/**
 * A price moved from one currency's base units to another's, through the SOL
 * rate — the only ratio the app knows. `null` while a rate it needs is missing,
 * so a change is refused rather than saved at a guessed price.
 */
function convertPrice(
  amount: number,
  from: SupportedCurrency,
  to: SupportedCurrency
): number | null {
  if (from === to) return amount
  const usd = fiatValueOf(from, amount, rates.value, 'usd')
  if (usd === null) return null
  if (to === 'usdc') return Math.max(1, Math.round(usd * 1_000_000))
  if (to === 'eurc') {
    const inEur = fiatValueOf('usdc', Math.round(usd * 1_000_000), rates.value, 'eur')
    return inEur === null ? null : Math.max(1, Math.round(inEur * 1_000_000))
  }
  const rate = rates.value?.usd
  if (!rate) return null
  return Math.max(1, Math.round((usd / rate) * 1_000_000_000))
}

async function load() {
  loading.value = true
  loadFailure.value = ''
  try {
    const shop = await api.get(props.shopId)
    await loadCategories()
    shopName.value = shop.name
    shopDescription.value = shop.description
    shopCurrency.value = shop.currency
    savedCurrency.value = shop.currency
    products.value = shop.products
  } catch (error) {
    loadFailure.value = apiErrorMessage(error, t('shopEditor.loadFailure'))
  } finally {
    loading.value = false
  }
}

function productBody(product: Product, price: number) {
  return {
    name: product.name,
    details: product.details,
    category: product.category,
    price_lamports: price
  }
}

/**
 * Saves the shop. Changing the currency changes what every stored price means,
 * so the catalogue is converted first — in the currency the prices were saved
 * in — and the shop only starts charging the new one once all of it has moved.
 * A failure puts back what was already rewritten, so a shop is never half in
 * one currency and half in the other.
 */
async function saveShop() {
  if (!shopName.value.trim() || savingShop.value) return
  savingShop.value = true
  failure.value = ''

  const previous = savedCurrency.value
  const next = shopCurrency.value
  const original = [...products.value]
  const converted: Product[] = []

  try {
    if (next !== previous && products.value.length) {
      if (!rates.value) throw new Error(t('shopEditor.ratesNeeded'))
      for (const product of original) {
        const price = convertPrice(product.price_lamports, previous, next)
        if (price === null) throw new Error(t('shopEditor.ratesNeeded'))
        const updated = await api.updateProduct(props.shopId, product.id, productBody(product, price))
        converted.push(product)
        products.value = products.value.map(item => (item.id === updated.id ? updated : item))
      }
    }

    const shop = await api.updateShop(props.shopId, {
      name: shopName.value.trim(),
      description: shopDescription.value.trim(),
      currency: next
    })
    shopName.value = shop.name
    shopDescription.value = shop.description
    shopCurrency.value = shop.currency
    savedCurrency.value = shop.currency
  } catch (error) {
    // Undo the prices this attempt already rewrote, oldest first.
    for (const product of converted) {
      await api.updateProduct(props.shopId, product.id, productBody(product, product.price_lamports))
        .catch(() => { /* the refresh below shows what the API actually kept */ })
    }
    products.value = original
    failure.value = converted.length
      ? t('shopEditor.currencyChangeFailure')
      : apiErrorMessage(error, t('shopEditor.saveFailure'))
  } finally {
    savingShop.value = false
  }
}

function resetForm() {
  editingId.value = null
  productName.value = ''
  productDetails.value = ''
  productCategory.value = ''
  productPrice.value = ''
  productFailure.value = ''
}

function openAdd() {
  resetForm()
  formOpen.value = true
}

/** Editing shows the stored price in the shop's own currency, unconverted. */
function openEdit(product: Product) {
  editingId.value = product.id
  productName.value = product.name
  productDetails.value = product.details
  productCategory.value = product.category
  productPrice.value = priceInputValue(product.price_lamports)
  productFailure.value = ''
  formOpen.value = true
}

function closeForm() {
  formOpen.value = false
  resetForm()
}

async function submitProduct() {
  if (!canSubmitProduct.value) return
  productBusy.value = true
  productFailure.value = ''
  try {
    const body = {
      name: productName.value.trim(),
      details: productDetails.value.trim(),
      category: productCategory.value.trim(),
      price_lamports: priceBaseUnits.value
    }
    // A heading the seller typed is remembered for this shop, so the next
    // product can pick it from the list instead of typing it again.
    if (body.category) {
      void api.addCategory(props.shopId, body.category)
        .then((names) => { categoryOptions.value = names })
        .catch(() => { /* the product saved; the suggestion can wait */ })
    }
    if (editingId.value) {
      const updated = await api.updateProduct(props.shopId, editingId.value, body)
      products.value = products.value.map(item => (item.id === updated.id ? updated : item))
    } else {
      products.value = [...products.value, await api.createProduct(props.shopId, body)]
    }
    closeForm()
  } catch (error) {
    productFailure.value = apiErrorMessage(error, t('shopEditor.productFailureTitle'))
  } finally {
    productBusy.value = false
  }
}

async function removeProduct(product: Product) {
  const confirmed = window.confirm(
    t('shopEditor.removeConfirm', { name: product.name })
  )
  if (!confirmed) return

  productFailure.value = ''
  try {
    await api.deleteProduct(props.shopId, product.id)
    products.value = products.value.filter(item => item.id !== product.id)
    if (editingId.value === product.id) closeForm()
  } catch (error) {
    productFailure.value = apiErrorMessage(error, t('shopEditor.productFailureTitle'))
  }
}

/** "1 product" / "2 products", in whatever forms the locale defines. */
function productCount(count: number) {
  return t('shopEditor.products', { count }, count)
}

onMounted(() => {
  void load()
})
</script>

<template>
  <div class="flex flex-col gap-6">
    <UAlert
      v-if="loadFailure"
      color="error"
      variant="subtle"
      icon="i-lucide-circle-alert"
      :title="$t('shopEditor.loadFailureTitle')"
      :description="loadFailure"
    />

    <p
      v-else-if="loading"
      class="py-8 text-center text-sm text-muted"
    >
      {{ $t('shopEditor.loading') }}
    </p>

    <template v-else>
      <UCard class="min-w-0">
        <template #header>
          <div class="flex items-center justify-between gap-3">
            <div>
              <h2 class="text-base font-semibold">
                {{ $t('shopEditor.detailsTitle') }}
              </h2>
              <p class="text-sm text-muted">
                {{ $t('shopEditor.detailsSubtitle') }}
              </p>
            </div>
            <UBadge
              color="neutral"
              variant="subtle"
              class="shrink-0 whitespace-nowrap"
            >
              {{ productCount(products.length) }}
            </UBadge>
          </div>
        </template>

        <div class="flex flex-col gap-4">
          <!-- Each field on its own row: the name and the description are two
               things to write, not one line to fill. -->
          <div class="flex flex-col gap-3">
            <UFormField :label="$t('shopEditor.nameLabel')">
              <UInput
                v-model="shopName"
                class="w-full"
                :maxlength="48"
              />
            </UFormField>
            <UFormField :label="$t('shopEditor.descriptionLabel')">
              <UInput
                v-model="shopDescription"
                class="w-full"
                :placeholder="$t('shopEditor.descriptionPlaceholder')"
                :maxlength="160"
              />
            </UFormField>

            <!-- One currency for the whole menu, saved with the shop: this is
                 the money every payment from it is charged in. -->
            <UFormField
              :label="$t('shopEditor.pricesInLabel')"
              :help="$t('shopEditor.pricesInHelp')"
            >
              <USelect
                v-model="shopCurrency"
                :items="currencyOptions"
                class="w-full sm:w-48"
                :aria-label="$t('shopEditor.priceUnitAria')"
              />
            </UFormField>
          </div>

          <UAlert
            v-if="failure"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            :title="$t('shopEditor.saveFailureTitle')"
            :description="failure"
          />

          <UButton
            :label="$t('shopEditor.saveShop')"
            icon="i-lucide-check"
            :loading="savingShop"
            :disabled="!shopName.trim() || savingShop"
            class="self-start"
            @click="saveShop"
          />
        </div>
      </UCard>

      <UCard class="min-w-0">
        <template #header>
          <div class="flex items-center justify-between gap-3">
            <div>
              <h2 class="text-base font-semibold">
                {{ $t('shopEditor.productsTitle') }}
              </h2>
              <p class="text-sm text-muted">
                {{ $t('shopEditor.productsSubtitle') }}
              </p>
              <p class="text-xs text-muted">
                {{ $t('shopEditor.pricesInCharged', { currency: unitLabel }) }}
              </p>
            </div>
            <!-- The way to add a product stays at the top of the list: at sixty
                 products the form at the bottom was a page of scrolling away. -->
            <UButton
              :label="$t('shopEditor.addProduct')"
              icon="i-lucide-plus"
              size="sm"
              class="shrink-0"
              @click="openAdd"
            />
          </div>
        </template>

        <div class="flex flex-col gap-4">
          <ul
            v-if="products.length"
            class="flex flex-col divide-y divide-default"
          >
            <li
              v-for="product in products"
              :key="product.id"
              class="flex min-w-0 items-start justify-between gap-3 py-2"
            >
              <div class="min-w-0">
                <p class="flex flex-wrap items-center gap-1.5 text-sm font-medium break-words">
                  {{ product.name }}
                  <UBadge
                    v-if="product.category"
                    color="neutral"
                    variant="subtle"
                    size="sm"
                  >
                    {{ product.category }}
                  </UBadge>
                </p>
                <p
                  v-if="product.details"
                  class="text-xs text-muted break-words"
                >
                  {{ product.details }}
                </p>
              </div>
              <div class="flex shrink-0 items-center gap-1.5">
                <div class="flex flex-col items-end">
                  <span class="text-sm tabular-nums">{{ priceInCurrency(product.price_lamports, shopCurrency) }}</span>
                  <span
                    v-if="priceHint(product.price_lamports)"
                    class="text-xs text-muted tabular-nums"
                  >
                    ≈ {{ priceHint(product.price_lamports) }}
                  </span>
                </div>
                <UButton
                  icon="i-lucide-pencil"
                  color="neutral"
                  variant="ghost"
                  size="xs"
                  square
                  :aria-label="$t('shopEditor.editAria', { name: product.name })"
                  @click="openEdit(product)"
                />
                <UButton
                  icon="i-lucide-trash-2"
                  color="error"
                  variant="ghost"
                  size="xs"
                  square
                  :aria-label="$t('shopEditor.removeAria', { name: product.name })"
                  @click="removeProduct(product)"
                />
              </div>
            </li>
          </ul>
          <p
            v-else
            class="text-sm text-muted"
          >
            {{ $t('shopEditor.emptyProducts') }}
          </p>

          <UAlert
            v-if="productFailure && !formOpen"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            :title="$t('shopEditor.productFailureTitle')"
            :description="productFailure"
          />
        </div>
      </UCard>
    </template>

    <!-- One product at a time, in a sheet over the list: adding or editing
         never pushes the catalogue down, and closing it loses nothing. -->
    <UModal
      v-model:open="formOpen"
      :title="editingId ? $t('shopEditor.editProductTitle') : $t('shopEditor.addProductTitle')"
    >
      <template #body>
        <div class="flex flex-col gap-3">
          <div class="grid gap-3 sm:grid-cols-2">
            <UFormField :label="$t('shopEditor.productLabel')">
              <UInput
                v-model="productName"
                class="w-full"
                :placeholder="$t('shopEditor.productPlaceholder')"
                :maxlength="32"
              />
            </UFormField>
            <UFormField :label="$t('shopEditor.priceLabel')">
              <div class="flex items-center gap-2">
                <UInput
                  v-model="productPrice"
                  type="number"
                  min="0"
                  :step="shopCurrency === 'sol' ? '0.001' : '0.01'"
                  class="min-w-0 flex-1"
                  :placeholder="$t('shopEditor.pricePlaceholder')"
                />
                <!-- The unit is the shop's, so it is written here rather than
                     offered again as a choice on every product. -->
                <span class="w-12 shrink-0 text-sm text-muted tabular-nums">{{ unitLabel }}</span>
              </div>
            </UFormField>
            <UFormField
              :label="$t('shopEditor.categoryLabel')"
              :help="$t('shopEditor.categoryHelp')"
              class="sm:col-span-2"
            >
              <div class="relative">
                <UInput
                  v-model="productCategory"
                  class="w-full"
                  :placeholder="$t('shopEditor.categoryPlaceholder')"
                  :maxlength="32"
                  :aria-expanded="categoryOpen"
                  aria-autocomplete="list"
                  @focus="categoryOpen = true"
                  @input="categoryOpen = true"
                  @keydown.esc="categoryOpen = false"
                />
                <!-- The shop's own headings, filtered as the seller types, each
                     with the way to stop suggesting it. -->
                <ul
                  v-if="categoryOpen && categorySuggestions.length"
                  class="absolute z-20 mt-1 w-full overflow-hidden rounded-lg border border-default bg-default shadow-lg"
                >
                  <li
                    v-for="name in categorySuggestions"
                    :key="name"
                    class="flex items-center justify-between gap-2 px-1"
                  >
                    <button
                      type="button"
                      class="min-w-0 flex-1 truncate px-2 py-2 text-left text-sm hover:bg-elevated"
                      @click="pickCategory(name)"
                    >
                      {{ name }}
                    </button>
                    <UButton
                      icon="i-lucide-trash-2"
                      color="neutral"
                      variant="ghost"
                      size="xs"
                      :aria-label="$t('shopEditor.categoryDelete', { name })"
                      @click.stop="deleteCategory(name)"
                    />
                  </li>
                </ul>
                <p
                  v-else-if="categoryOpen"
                  class="mt-1 text-xs text-muted"
                >
                  {{ $t('shopEditor.categoryNew') }}
                </p>
              </div>
            </UFormField>
            <UFormField
              :label="$t('shopEditor.detailsLabel')"
              :help="$t('shopEditor.detailsHelp')"
              class="sm:col-span-2"
            >
              <UInput
                v-model="productDetails"
                class="w-full"
                :placeholder="$t('shopEditor.detailsPlaceholder')"
                :maxlength="160"
              />
            </UFormField>
          </div>

          <p
            v-if="shopCurrency !== 'sol' && !rates"
            class="text-xs text-muted"
          >
            {{ $t('shopEditor.readingRate') }}
          </p>

          <UAlert
            v-if="productFailure"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            :title="$t('shopEditor.productFailureTitle')"
            :description="productFailure"
          />
        </div>
      </template>

      <template #footer>
        <div class="flex w-full items-center justify-end gap-2">
          <UButton
            :label="$t('common.cancel')"
            color="neutral"
            variant="ghost"
            @click="closeForm"
          />
          <UButton
            :label="editingId ? $t('shopEditor.saveChanges') : $t('shopEditor.addProduct')"
            :icon="editingId ? 'i-lucide-check' : 'i-lucide-plus'"
            :loading="productBusy"
            :disabled="!canSubmitProduct"
            @click="submitProduct"
          />
        </div>
      </template>
    </UModal>
  </div>
</template>
