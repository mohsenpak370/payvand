<script setup lang="ts">
import type { ShopWithProducts } from '~/composables/useShop'
import type { SupportedCurrency } from '~/composables/usePaymentApi'

// A seller may keep several shops — a café and a bakery. The /shops page lists
// them; each one opens on its own page, where its catalogue is edited. The
// form lives in a modal so an empty page is not a form to fill in by accident.
const { t } = useI18n()
const api = useShopApi()

const shops = ref<ShopWithProducts[]>([])
const loading = ref(true)
const failure = ref('')

const name = ref('')
const description = ref('')
const creating = ref(false)
const formOpen = ref(false)

// The same purchase the card above the list shows: one status, one price list.
const purchase = useSubscriptionPurchase()
/** The dialog is standing where the subscription offer belongs instead. */
const needsSubscription = ref(false)
/** The form came back filled from a payment this browser left to make. */
const resumed = ref(false)

const canCreate = computed(() => !creating.value && name.value.trim().length > 0)

const SHOP_DRAFT_KEY = 'payvand.shop-draft.v1'
/** A draft older than this is a form abandoned, not a payment in flight. */
const DRAFT_TTL_MS = 2 * 60 * 60 * 1000
interface ShopDraft { name: string, description: string, at: number }

/** The shop being written when the dialog sent the person off to pay. */
function saveDraft(): void {
  try {
    localStorage.setItem(SHOP_DRAFT_KEY, JSON.stringify({
      name: name.value,
      description: description.value,
      at: Date.now()
    }))
  } catch {
    // Private mode without storage: the payment still goes ahead, and the two
    // fields are typed again when the person comes back.
  }
}

function readDraft(): ShopDraft | null {
  try {
    const raw = localStorage.getItem(SHOP_DRAFT_KEY)
    if (!raw) return null
    const draft = JSON.parse(raw) as ShopDraft
    if (!draft.name || Date.now() - draft.at > DRAFT_TTL_MS) {
      localStorage.removeItem(SHOP_DRAFT_KEY)
      return null
    }
    return draft
  } catch {
    return null
  }
}

function clearDraft(): void {
  try {
    localStorage.removeItem(SHOP_DRAFT_KEY)
  } catch {
    // Nothing to clear.
  }
}

/** A 403 when creating a shop is the subscription gate, and nothing else. */
function requiresSubscription(error: unknown): boolean {
  if (typeof error === 'object' && error !== null) {
    const code = (error as { statusCode?: number, status?: number }).statusCode
      ?? (error as { status?: number }).status
    if (code === 403) return true
  }
  return apiErrorMessage(error, '').toLowerCase().includes('subscription')
}

/**
 * Keeps the draft, then opens the payment sheet over this page. The dialog
 * closes first so the sheet is not stacked underneath it, and comes back —
 * still filled in, from the draft — once the sheet is gone.
 */
async function startSubscription(currency: SupportedCurrency): Promise<void> {
  saveDraft()
  formOpen.value = false
  await purchase.pay(currency)
  // No invoice, no sheet: the dialog comes back to say why.
  if (!purchase.checkout.value && purchase.failure.value) formOpen.value = true
}

/**
 * The payment sheet has closed. A shop that was half written comes back with
 * its two fields intact; the offer that sent the person to pay is gone if the
 * payment went through, and still stands if it did not.
 */
watch(
  () => purchase.checkout.value,
  async (current, previous) => {
    if (!previous || current) return
    const draft = readDraft()
    if (!draft) return
    name.value = draft.name
    description.value = draft.description
    // The shared status, read again: `load` confirms the invoice that was just
    // paid before it answers whether the account is active.
    await purchase.load()
    const paid = purchase.active.value
    if (paid) clearDraft()
    resumed.value = paid
    needsSubscription.value = !paid
    formOpen.value = true
  }
)

async function load() {
  loading.value = true
  failure.value = ''
  try {
    shops.value = await api.list()
  } catch (error) {
    failure.value = apiErrorMessage(error, t('shopsList.loadFailure'))
  } finally {
    loading.value = false
  }
}

function openForm() {
  failure.value = ''
  resumed.value = false
  // A status already read says whether the offer belongs here from the start.
  needsSubscription.value = purchase.status.value ? !purchase.status.value.active : false
  formOpen.value = true
}

async function createShop() {
  if (!canCreate.value) return
  creating.value = true
  failure.value = ''
  try {
    const shop = await api.createShop({
      name: name.value.trim(),
      description: description.value.trim()
    })
    // Straight to the new shop, which is empty and waiting for products.
    clearDraft()
    formOpen.value = false
    await navigateTo(`/shops/${shop.id}`)
  } catch (error) {
    failure.value = apiErrorMessage(error, t('shopsList.createFailure'))
    if (requiresSubscription(error)) {
      needsSubscription.value = true
      void purchase.load()
    }
  } finally {
    creating.value = false
  }
}

/** "1 product" / "2 products", in whatever forms the locale defines. */
function productCount(count: number) {
  return t('shopsList.products', { count }, count)
}

onMounted(async () => {
  await load()
  const draft = readDraft()
  if (!draft) return
  name.value = draft.name
  description.value = draft.description
  // Shared with the card above, and where a payment made in the checkout comes
  // back: `load` confirms the pending invoice before it answers `active`.
  await purchase.load()
  if (!purchase.status.value?.active) return
  clearDraft()
  resumed.value = true
  needsSubscription.value = false
  formOpen.value = true
})
</script>

<template>
  <UCard class="min-w-0">
    <template #header>
      <div class="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between sm:gap-3">
        <div class="min-w-0">
          <h2 class="text-base font-semibold whitespace-nowrap">
            {{ $t('shopsList.title') }}
          </h2>
          <p class="text-sm text-muted">
            {{ $t('shopsList.subtitle') }}
          </p>
        </div>
        <UButton
          :label="$t('shopsList.newShop')"
          icon="i-lucide-plus"
          size="sm"
          class="shrink-0 self-start"
          @click="openForm"
        />
      </div>
    </template>

    <div class="flex flex-col gap-4">
      <UAlert
        v-if="failure && !formOpen"
        color="error"
        variant="subtle"
        icon="i-lucide-circle-alert"
        :title="$t('shopsList.errorTitle')"
        :description="failure"
      />

      <p
        v-if="loading"
        class="py-4 text-center text-sm text-muted"
      >
        {{ $t('shopsList.loading') }}
      </p>

      <template v-else>
        <ul
          v-if="shops.length"
          class="flex flex-col divide-y divide-default"
        >
          <li
            v-for="shop in shops"
            :key="shop.id"
            class="flex min-w-0 items-center justify-between gap-3 py-3"
          >
            <div class="min-w-0">
              <p class="text-sm font-medium break-words">
                {{ shop.name }}
              </p>
              <p
                v-if="shop.description"
                class="text-xs text-muted break-words"
              >
                {{ shop.description }}
              </p>
              <!-- One line even on a narrow phone: "2 products" is a count, not
                   a phrase worth wrapping. -->
              <p class="mt-0.5 text-xs whitespace-nowrap text-muted">
                {{ productCount(shop.products.length) }}
              </p>
            </div>
            <UButton
              :to="`/shops/${shop.id}`"
              :label="$t('shopsList.openShop')"
              icon="i-lucide-arrow-right"
              trailing
              color="neutral"
              variant="soft"
              size="sm"
            />
          </li>
        </ul>

        <p
          v-else
          class="text-sm text-muted"
        >
          {{ $t('shopsList.empty') }}
        </p>
      </template>
    </div>

    <UModal
      v-model:open="formOpen"
      :title="$t('shopsList.newShop')"
      :description="$t('shopsList.modalDescription')"
      :ui="{ content: 'max-w-md' }"
    >
      <template #body>
        <div class="flex flex-col gap-4">
          <UFormField :label="$t('shopsList.nameLabel')">
            <UInput
              v-model="name"
              class="w-full"
              :placeholder="$t('shopsList.namePlaceholder')"
              :maxlength="48"
              autofocus
            />
          </UFormField>
          <UFormField
            :label="$t('shopsList.descriptionLabel')"
            :help="$t('shopsList.descriptionHelp')"
          >
            <UInput
              v-model="description"
              class="w-full"
              :placeholder="$t('shopsList.descriptionPlaceholder')"
              :maxlength="160"
            />
          </UFormField>

          <UAlert
            v-if="resumed"
            color="success"
            variant="subtle"
            icon="i-lucide-circle-check"
            :description="$t('shopsList.subscriptionResume')"
          />

          <UAlert
            v-if="failure && !needsSubscription"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            :title="$t('shopsList.createFailure')"
            :description="failure"
          />

          <!-- The offer the subscription gate asks for, in the dialog rather
               than a trip to another page: price, then the currency, then the
               wallet. -->
          <div
            v-if="needsSubscription"
            class="border-default flex flex-col gap-3 rounded-lg border p-3"
          >
            <div>
              <p class="text-sm font-medium">
                {{ $t('shopsList.subscriptionNeededTitle') }}
              </p>
              <p class="text-xs text-muted">
                {{ $t('shopsList.subscriptionNeededDescription') }}
              </p>
            </div>

            <p
              v-if="purchase.status.value"
              class="text-sm text-muted"
            >
              {{ $t('subscription.price', { price: '€1.70', days: purchase.status.value.period_days }) }}
            </p>

            <p
              v-if="purchase.status.value && !purchase.status.value.rates_available"
              class="text-sm text-muted"
            >
              {{ $t('subscription.ratesUnavailable') }}
            </p>
            <div
              v-else
              class="flex flex-wrap gap-2"
            >
              <UButton
                v-for="quote in purchase.quotes.value"
                :key="quote.currency"
                :label="purchase.priceLabel(quote)"
                :loading="purchase.paying.value === quote.currency"
                :disabled="purchase.paying.value !== '' || !purchase.status.value"
                color="neutral"
                variant="soft"
                size="sm"
                :icon="purchase.covered.value === quote.currency ? 'i-lucide-zap' : 'i-lucide-credit-card'"
                @click="startSubscription(quote.currency)"
              />
            </div>

            <p
              v-if="purchase.covered.value"
              class="text-xs text-muted"
            >
              {{ $t('subscription.fromAllowance') }}
            </p>

            <p
              v-if="purchase.failure.value"
              class="text-xs text-error"
            >
              {{ purchase.failure.value }}
            </p>
          </div>
        </div>
      </template>

      <template #footer>
        <div class="flex w-full items-center justify-end gap-2">
          <UButton
            :label="$t('common.cancel')"
            color="neutral"
            variant="ghost"
            @click="formOpen = false"
          />
          <UButton
            v-if="!needsSubscription"
            :label="$t('shopsList.createShop')"
            icon="i-lucide-store"
            :loading="creating"
            :disabled="!canCreate"
            @click="createShop"
          />
        </div>
      </template>
    </UModal>
  </UCard>
</template>
