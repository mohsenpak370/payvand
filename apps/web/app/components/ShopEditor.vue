<script setup lang="ts">
import type { Product } from '~/composables/useShop'

// One shop's page: its name, its description, and what it sells. Everything
// here is the seller's own menu — the payment page reads the same catalogue
// and turns a cart of it into a receipt.
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

// Products are stored in lamports and charged in SOL; the seller may type them
// in the money they think in. That choice belongs to the shop rather than to
// each product line: a menu is written in one currency, and asking it again for
// every item is a question with one answer.
const { currency } = useCurrencyPreference()
const rates = useSolRates()
type PriceUnit = 'sol' | 'usd' | 'eur'
const priceUnits: { label: string, value: PriceUnit }[] = [
  { label: 'SOL', value: 'sol' },
  { label: 'USD', value: 'usd' },
  { label: 'EUR', value: 'eur' }
]

/**
 * Kept in the browser next to the account's own display currency: the unit
 * changes nothing about the shop — its prices are lamports either way — so this
 * is how they are written and read back, not shop data to sync.
 */
const PRICE_UNIT_KEY = 'payvand.shop-price-unit.v1'

const priceUnit = ref<PriceUnit>('sol')

function readPriceUnit(): PriceUnit {
  try {
    const stored = localStorage.getItem(`${PRICE_UNIT_KEY}:${props.shopId}`)
    return stored === 'usd' || stored === 'eur' ? stored : 'sol'
  } catch {
    return 'sol'
  }
}

watch(priceUnit, (unit) => {
  try {
    localStorage.setItem(`${PRICE_UNIT_KEY}:${props.shopId}`, unit)
  } catch {
    // Private mode without storage: the choice still holds for this visit.
  }
})

/** The unit as it is written after a price field. */
const unitLabel = computed(() =>
  priceUnit.value === 'sol' ? 'SOL' : priceUnit.value.toUpperCase()
)

const editingId = ref<string | null>(null)
const productName = ref('')
const productDetails = ref('')
const productCategory = ref('')
const productPrice = ref('')
const productBusy = ref(false)
const productFailure = ref('')

const priceLamports = computed(() =>
  priceUnit.value === 'sol'
    ? solToLamports(Number(productPrice.value))
    : lamportsFromFiat(Number(productPrice.value), rates.value, priceUnit.value)
)

const canSubmitProduct = computed(() =>
  !productBusy.value
  && productName.value.trim().length > 0
  && priceLamports.value > 0
)

/** The price in the reader's own money, when the rate is known. */
function fiatHint(lamports: number): string | null {
  return fiatFromLamports(lamports, rates.value, currency.value)
}

/** A stored price, written the way this shop writes its prices. */
function priceLabel(lamports: number): string {
  if (priceUnit.value === 'sol') return `${lamportsToSol(lamports)} SOL`
  return fiatFromLamports(lamports, rates.value, priceUnit.value)
    ?? `${lamportsToSol(lamports)} SOL`
}

/**
 * The figure under the shop's own: in SOL that is the reader's money, and in
 * dollars or euros it is the SOL the customer actually hands over, so the two
 * lines are never the same amount twice.
 */
function priceHint(lamports: number): string | null {
  if (priceUnit.value === 'sol') return fiatHint(lamports)
  return `${lamportsToSol(lamports)} SOL`
}

/** A stored price in the shop's unit, as a number for the edit field. */
function priceInputValue(lamports: number): string {
  if (priceUnit.value === 'sol') return lamportsToSol(lamports)
  const rate = rates.value?.[priceUnit.value]
  // No rate yet: leave it to be typed rather than show a figure in a unit the
  // seller did not pick.
  if (!rate) return ''
  return ((lamports / 1_000_000_000) * rate).toFixed(2)
}

async function load() {
  loading.value = true
  loadFailure.value = ''
  try {
    const shop = await api.get(props.shopId)
    await loadCategories()
    shopName.value = shop.name
    shopDescription.value = shop.description
    products.value = shop.products
  } catch (error) {
    loadFailure.value = apiErrorMessage(error, t('shopEditor.loadFailure'))
  } finally {
    loading.value = false
  }
}

async function saveShop() {
  if (!shopName.value.trim()) return
  savingShop.value = true
  failure.value = ''
  try {
    const shop = await api.updateShop(props.shopId, {
      name: shopName.value.trim(),
      description: shopDescription.value.trim()
    })
    shopName.value = shop.name
    shopDescription.value = shop.description
  } catch (error) {
    failure.value = apiErrorMessage(error, t('shopEditor.saveFailure'))
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
  // The shop's unit is a setting, not part of the form: clearing the form
  // leaves it as the seller chose it.
  productFailure.value = ''
}

/** Editing shows the stored price in SOL: a converted figure could drift. */
function editProduct(product: Product) {
  editingId.value = product.id
  productName.value = product.name
  productDetails.value = product.details
  productCategory.value = product.category
  productPrice.value = priceInputValue(product.price_lamports)
  productFailure.value = ''
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
      price_lamports: priceLamports.value
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
    resetForm()
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
    if (editingId.value === product.id) resetForm()
  } catch (error) {
    productFailure.value = apiErrorMessage(error, t('shopEditor.productFailureTitle'))
  }
}

/** "1 product" / "2 products", in whatever forms the locale defines. */
function productCount(count: number) {
  return t('shopEditor.products', { count }, count)
}

onMounted(() => {
  priceUnit.value = readPriceUnit()
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

            <!-- One currency for the whole menu, chosen with the shop rather
                 than on every product. -->
            <UFormField
              :label="$t('shopEditor.pricesInLabel')"
              :help="$t('shopEditor.pricesInHelp')"
            >
              <USelect
                v-model="priceUnit"
                :items="priceUnits"
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
            </div>
            <span class="text-xs text-muted">{{ $t('shopEditor.pricesInSol') }}</span>
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
                  <span class="text-sm tabular-nums">{{ priceLabel(product.price_lamports) }}</span>
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
                  @click="editProduct(product)"
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

          <div class="flex flex-col gap-3 border-t border-default pt-4">
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
                    :step="priceUnit === 'sol' ? '0.001' : '0.01'"
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
              v-if="priceUnit !== 'sol' && !rates"
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

            <div class="flex flex-wrap items-center gap-2">
              <UButton
                :label="editingId ? $t('shopEditor.saveChanges') : $t('shopEditor.addProduct')"
                :icon="editingId ? 'i-lucide-check' : 'i-lucide-plus'"
                :loading="productBusy"
                :disabled="!canSubmitProduct"
                @click="submitProduct"
              />
              <UButton
                v-if="editingId"
                :label="$t('common.cancel')"
                color="neutral"
                variant="ghost"
                @click="resetForm"
              />
            </div>
          </div>
        </div>
      </UCard>
    </template>
  </div>
</template>
