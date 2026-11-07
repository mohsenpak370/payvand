<script setup lang="ts">
import type { Product } from '~/composables/useShop'

// One shop's page: its name, its description, and what it sells. Everything
// here is the seller's own menu — the payment page reads the same catalogue
// and turns a cart of it into a receipt.
const props = defineProps<{ shopId: string }>()

const api = useShopApi()

const shopName = ref('')
const shopDescription = ref('')
const products = ref<Product[]>([])

const loading = ref(true)
const savingShop = ref(false)
/** Set when the shop itself could not be opened — the page shows nothing else. */
const loadFailure = ref('')
const failure = ref('')

// Prices are paid in SOL; the seller may type them in the money they think in.
// The unit is a convenience at the keyboard — what is stored is always SOL.
const { currency } = useCurrencyPreference()
const rates = useSolRates()
type PriceUnit = 'sol' | 'usd' | 'eur'
const priceUnits: { label: string, value: PriceUnit }[] = [
  { label: 'SOL', value: 'sol' },
  { label: 'USD', value: 'usd' },
  { label: 'EUR', value: 'eur' }
]

const editingId = ref<string | null>(null)
const productName = ref('')
const productDetails = ref('')
const productPrice = ref('')
const priceUnit = ref<PriceUnit>('sol')
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

async function load() {
  loading.value = true
  loadFailure.value = ''
  try {
    const shop = await api.get(props.shopId)
    shopName.value = shop.name
    shopDescription.value = shop.description
    products.value = shop.products
  } catch (error) {
    loadFailure.value = apiErrorMessage(error, 'Could not open this shop.')
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
    failure.value = apiErrorMessage(error, 'Could not save the shop.')
  } finally {
    savingShop.value = false
  }
}

function resetForm() {
  editingId.value = null
  productName.value = ''
  productDetails.value = ''
  productPrice.value = ''
  priceUnit.value = 'sol'
  productFailure.value = ''
}

/** Editing shows the stored price in SOL: a converted figure could drift. */
function editProduct(product: Product) {
  editingId.value = product.id
  productName.value = product.name
  productDetails.value = product.details
  productPrice.value = lamportsToSol(product.price_lamports)
  priceUnit.value = 'sol'
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
      price_lamports: priceLamports.value
    }
    if (editingId.value) {
      const updated = await api.updateProduct(props.shopId, editingId.value, body)
      products.value = products.value.map(item => (item.id === updated.id ? updated : item))
    } else {
      products.value = [...products.value, await api.createProduct(props.shopId, body)]
    }
    resetForm()
  } catch (error) {
    productFailure.value = apiErrorMessage(error, 'Could not save the product.')
  } finally {
    productBusy.value = false
  }
}

async function removeProduct(product: Product) {
  const confirmed = window.confirm(
    `Remove “${product.name}”?\n\nInvoices already created keep their receipt — this only takes it off the menu.`
  )
  if (!confirmed) return

  productFailure.value = ''
  try {
    await api.deleteProduct(props.shopId, product.id)
    products.value = products.value.filter(item => item.id !== product.id)
    if (editingId.value === product.id) resetForm()
  } catch (error) {
    productFailure.value = apiErrorMessage(error, 'Could not remove the product.')
  }
}

onMounted(load)
</script>

<template>
  <div class="flex flex-col gap-6">
    <UAlert
      v-if="loadFailure"
      color="error"
      variant="subtle"
      icon="i-lucide-circle-alert"
      title="Could not open this shop"
      :description="loadFailure"
    />

    <p
      v-else-if="loading"
      class="py-8 text-center text-sm text-muted"
    >
      Reading your shop…
    </p>

    <template v-else>
      <UCard class="min-w-0">
        <template #header>
          <div class="flex items-center justify-between gap-3">
            <div>
              <h2 class="text-base font-semibold">
                Shop details
              </h2>
              <p class="text-sm text-muted">
                What this shop is called, and what it sells in a line.
              </p>
            </div>
            <UBadge
              color="neutral"
              variant="subtle"
              class="shrink-0 whitespace-nowrap"
            >
              {{ products.length }} {{ products.length === 1 ? 'product' : 'products' }}
            </UBadge>
          </div>
        </template>

        <div class="flex flex-col gap-4">
          <!-- Each field on its own row: the name and the description are two
               things to write, not one line to fill. -->
          <div class="flex flex-col gap-3">
            <UFormField label="Shop name">
              <UInput
                v-model="shopName"
                class="w-full"
                :maxlength="48"
              />
            </UFormField>
            <UFormField label="Description">
              <UInput
                v-model="shopDescription"
                class="w-full"
                placeholder="Flat whites and fresh bread"
                :maxlength="160"
              />
            </UFormField>
          </div>

          <UAlert
            v-if="failure"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            title="Could not save the shop"
            :description="failure"
          />

          <UButton
            label="Save shop"
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
                Products
              </h2>
              <p class="text-sm text-muted">
                What the payment page offers as a receipt.
              </p>
            </div>
            <span class="text-xs text-muted">Prices are charged in SOL</span>
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
                <p class="text-sm font-medium break-words">
                  {{ product.name }}
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
                  <span class="text-sm tabular-nums">{{ lamportsToSol(product.price_lamports) }} SOL</span>
                  <span
                    v-if="fiatHint(product.price_lamports)"
                    class="text-xs text-muted tabular-nums"
                  >
                    ≈ {{ fiatHint(product.price_lamports) }}
                  </span>
                </div>
                <UButton
                  icon="i-lucide-pencil"
                  color="neutral"
                  variant="ghost"
                  size="xs"
                  square
                  :aria-label="`Edit ${product.name}`"
                  @click="editProduct(product)"
                />
                <UButton
                  icon="i-lucide-trash-2"
                  color="error"
                  variant="ghost"
                  size="xs"
                  square
                  :aria-label="`Remove ${product.name}`"
                  @click="removeProduct(product)"
                />
              </div>
            </li>
          </ul>
          <p
            v-else
            class="text-sm text-muted"
          >
            No products yet — add the first one below.
          </p>

          <div class="flex flex-col gap-3 border-t border-default pt-4">
            <div class="grid gap-3 sm:grid-cols-2">
              <UFormField label="Product">
                <UInput
                  v-model="productName"
                  class="w-full"
                  placeholder="Flat white"
                  :maxlength="32"
                />
              </UFormField>
              <UFormField label="Price">
                <div class="flex items-stretch gap-2">
                  <UInput
                    v-model="productPrice"
                    type="number"
                    min="0"
                    :step="priceUnit === 'sol' ? '0.001' : '0.01'"
                    class="min-w-0 flex-1"
                    placeholder="0.05"
                  />
                  <USelect
                    v-model="priceUnit"
                    :items="priceUnits"
                    class="w-24"
                    aria-label="Unit of the price"
                  />
                </div>
              </UFormField>
              <UFormField
                label="Details"
                help="Optional — size, what is in it, anything the customer should know."
                class="sm:col-span-2"
              >
                <UInput
                  v-model="productDetails"
                  class="w-full"
                  placeholder="Large, oat milk"
                  :maxlength="160"
                />
              </UFormField>
            </div>

            <p
              v-if="priceUnit !== 'sol' && !rates"
              class="text-xs text-muted"
            >
              Reading today's rate…
            </p>

            <UAlert
              v-if="productFailure"
              color="error"
              variant="subtle"
              icon="i-lucide-circle-alert"
              title="Could not save the product"
              :description="productFailure"
            />

            <div class="flex flex-wrap items-center gap-2">
              <UButton
                :label="editingId ? 'Save changes' : 'Add product'"
                :icon="editingId ? 'i-lucide-check' : 'i-lucide-plus'"
                :loading="productBusy"
                :disabled="!canSubmitProduct"
                @click="submitProduct"
              />
              <UButton
                v-if="editingId"
                label="Cancel"
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
