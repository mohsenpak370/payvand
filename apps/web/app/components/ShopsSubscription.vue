<script setup lang="ts">
import type { SubscriptionStatus } from '~/composables/useSubscription'
import {
  currencySymbol,
  formatTokenAmount,
  lamportsToSol,
  paymentLinkPath,
  type SupportedCurrency
} from '~/composables/usePaymentApi'

// The shops feature is sold by the month: €1.70 for 30 days, in SOL, USDC or
// EURC. The three amounts are the API's quote, converted at its live rate, so
// what this card shows is exactly what the invoice charges. Paying opens the
// ordinary checkout with a platform invoice; coming back, the pending payment id
// is confirmed against the cluster.
const { t } = useI18n()
const api = useSubscription()

const status = ref<SubscriptionStatus | null>(null)
const loading = ref(true)
const failure = ref('')
const paying = ref<SupportedCurrency | ''>('')

/** A price in the money the customer hands over. */
function priceLabel(quote: { currency: SupportedCurrency, amount: number }): string {
  return quote.currency === 'sol'
    ? `${lamportsToSol(quote.amount)} SOL`
    : `${formatTokenAmount(quote.amount, 6)} ${currencySymbol(quote.currency)}`
}

const expiry = computed(() => {
  const seconds = status.value?.expires_at_unix ?? 0
  if (!seconds) return ''
  return new Date(seconds * 1000).toLocaleDateString()
})

async function load() {
  loading.value = true
  failure.value = ''
  try {
    status.value = await api.status()
  } catch (error) {
    failure.value = apiErrorMessage(error, t('subscription.loadFailure'))
  } finally {
    loading.value = false
  }
}

/** Confirms the invoice this page sent the seller to pay, if there is one. */
async function confirmPending() {
  const pending = api.takePending()
  if (!pending) return
  try {
    status.value = await api.confirm(pending)
  } catch {
    // Not settled yet, or the cluster has not caught up: the id is spent and
    // the status reload below tells the seller where they stand.
    await load()
  }
}

async function pay(currency: SupportedCurrency) {
  if (paying.value) return
  paying.value = currency
  failure.value = ''
  try {
    const invoice = await api.startPayment(currency)
    api.rememberPending(invoice.payment_id)
    await navigateTo(paymentLinkPath(invoice.payload))
  } catch (error) {
    failure.value = apiErrorMessage(error, t('subscription.payFailure'))
  } finally {
    paying.value = ''
  }
}

onMounted(async () => {
  await confirmPending()
  await load()
})
</script>

<template>
  <UCard class="min-w-0">
    <template #header>
      <div class="flex items-start justify-between gap-3">
        <div class="min-w-0">
          <h2 class="text-base font-semibold">
            {{ $t('subscription.title') }}
          </h2>
          <p class="text-sm text-muted">
            {{ $t('subscription.description') }}
          </p>
        </div>
        <UBadge
          v-if="status"
          :color="status.active ? 'success' : 'neutral'"
          variant="subtle"
          class="shrink-0"
        >
          {{ status.active ? $t('subscription.active') : $t('subscription.inactive') }}
        </UBadge>
      </div>
    </template>

    <div class="flex flex-col gap-4">
      <UAlert
        v-if="failure"
        color="error"
        variant="subtle"
        icon="i-lucide-circle-alert"
        :title="$t('subscription.errorTitle')"
        :description="failure"
      />

      <p
        v-if="loading"
        class="py-2 text-center text-sm text-muted"
      >
        {{ $t('subscription.loading') }}
      </p>

      <template v-else-if="status">
        <p
          v-if="status.active"
          class="text-sm text-muted"
        >
          {{ $t('subscription.renewsOn', { date: expiry }) }}
        </p>
        <p
          v-else
          class="text-sm text-muted"
        >
          {{ $t('subscription.price', { price: '€1.70', days: status.period_days }) }}
        </p>

        <div class="flex flex-wrap gap-2">
          <UButton
            v-for="quote in status.quotes"
            :key="quote.currency"
            :label="priceLabel(quote)"
            :loading="paying === quote.currency"
            :disabled="paying !== ''"
            color="neutral"
            variant="soft"
            size="sm"
            icon="i-lucide-credit-card"
            @click="pay(quote.currency)"
          />
        </div>

        <p class="text-xs text-muted">
          {{ $t('subscription.rateNote', { date: new Date(status.rated_at_unix * 1000).toLocaleString() }) }}
        </p>
      </template>
    </div>
  </UCard>
</template>
