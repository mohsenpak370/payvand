<script setup lang="ts">
import type { SupportedCurrency } from '~/composables/usePaymentApi'

// The shops feature is sold by the month: €1.70 for 30 days, in SOL, USDC or
// EURC. The purchase lives in `useSubscriptionPurchase` because the New shop
// dialog offers the same one; this card is where the account's standing and
// the price live.
const purchase = useSubscriptionPurchase()
const { t } = useI18n()

const expiry = computed(() => {
  const seconds = purchase.status.value?.expires_at_unix ?? 0
  if (!seconds) return ''
  return new Date(seconds * 1000).toLocaleDateString()
})

/**
 * Whole days of the paid period still to run, rounded up: the last partial day
 * is still a day the seller has, so an active subscription never reads as
 * "0 days left" before it has actually run out.
 */
const daysLeft = computed(() => {
  const seconds = purchase.status.value?.expires_at_unix ?? 0
  if (!seconds) return 0
  return Math.max(0, Math.ceil((seconds * 1000 - Date.now()) / 86_400_000))
})

const quotes = computed(() => purchase.quotes.value)
/** The currency an allowance already pays for, if this device has one. */
const covered = computed(() => purchase.covered.value)

/**
 * What the buy button will charge. It opens on the currency an allowance covers
 * — the one that needs no wallet approval — and otherwise on the first price.
 * Prices arrive with the status, so an empty pick is filled when they land, but
 * a pick already made is never overridden.
 */
const selected = ref<SupportedCurrency | ''>('')

watch([quotes, covered], ([list, free]) => {
  if (selected.value && list.some(quote => quote.currency === selected.value)) return
  selected.value = free ?? list[0]?.currency ?? ''
}, { immediate: true })

const selectedQuote = computed(
  () => quotes.value.find(quote => quote.currency === selected.value) ?? null
)

/** The one button says both what it buys and what it costs. */
const buyLabel = computed(() => {
  const quote = selectedQuote.value
  return quote ? t('subscription.buy', { price: purchase.priceLabel(quote) }) : ''
})

function buy(): void {
  if (!selected.value || purchase.paying.value) return
  void purchase.pay(selected.value)
}

onMounted(() => {
  void purchase.load()
})
</script>

<template>
  <UCard class="min-w-0">
    <template #header>
      <div class="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between sm:gap-3">
        <div class="min-w-0">
          <h2 class="text-base font-semibold whitespace-nowrap">
            {{ $t('subscription.title') }}
          </h2>
          <p class="text-sm text-muted">
            {{ $t('subscription.description') }}
          </p>
        </div>
        <UBadge
          v-if="purchase.status.value"
          :color="purchase.status.value.active ? 'success' : 'neutral'"
          variant="subtle"
          class="shrink-0 self-start"
        >
          {{ purchase.status.value.active ? $t('subscription.active') : $t('subscription.inactive') }}
        </UBadge>
      </div>
    </template>

    <div class="flex flex-col gap-4">
      <UAlert
        v-if="purchase.failure.value"
        color="error"
        variant="subtle"
        icon="i-lucide-circle-alert"
        :title="$t('subscription.errorTitle')"
        :description="purchase.failure.value"
      />

      <p
        v-if="purchase.loading.value"
        class="py-2 text-center text-sm text-muted"
      >
        {{ $t('subscription.loading') }}
      </p>

      <template v-else-if="purchase.status.value">
        <!-- What the money bought: how much of the paid month is left, and the
             day it runs out. An active subscription answers with its own
             period instead of leaving the card's body empty. -->
        <div
          v-if="purchase.status.value.active"
          class="border-default rounded-lg border p-3"
        >
          <p class="text-sm font-medium">
            {{ $t('subscription.daysLeft', { count: daysLeft }, daysLeft) }}
          </p>
          <p class="text-muted mt-1 text-xs">
            {{ $t('subscription.renewsOn', { date: expiry }) }}
          </p>
        </div>
        <p
          v-else
          class="text-sm text-muted"
        >
          {{ $t('subscription.price', { price: '€1.70', days: purchase.status.value.period_days }) }}
        </p>

        <!-- No rate, no price: the status above still stands, and only the
             purchase waits until the next read answers. -->
        <p
          v-if="!purchase.status.value.rates_available"
          class="text-sm text-muted"
        >
          {{ $t('subscription.ratesUnavailable') }}
        </p>

        <template v-else>
          <!-- A choice, not three buttons: each currency is a card that is
               picked, and the one button below pays whatever is picked. -->
          <fieldset class="flex flex-col">
            <legend class="mb-2 text-xs font-medium tracking-wide text-muted uppercase">
              {{ $t('subscription.chooseCurrency') }}
            </legend>
            <div class="grid gap-2 sm:grid-cols-3">
              <label
                v-for="quote in quotes"
                :key="quote.currency"
                class="relative flex cursor-pointer items-start gap-2.5 rounded-lg border p-3 transition"
                :class="selected === quote.currency
                  ? 'border-primary bg-primary/5 ring-primary/30 ring-2'
                  : 'border-default hover:border-primary/40'"
              >
                <input
                  v-model="selected"
                  type="radio"
                  name="subscription-currency"
                  :value="quote.currency"
                  class="sr-only"
                  :disabled="purchase.paying.value !== ''"
                >
                <span
                  aria-hidden="true"
                  class="mt-0.5 flex size-4 shrink-0 items-center justify-center rounded-full border-2"
                  :class="selected === quote.currency ? 'border-primary' : 'border-muted'"
                >
                  <span
                    v-if="selected === quote.currency"
                    class="bg-primary size-2 rounded-full"
                  />
                </span>
                <span class="flex min-w-0 flex-col">
                  <span class="text-sm font-medium">
                    {{ currencySymbol(quote.currency) }}
                  </span>
                  <span class="text-xs whitespace-nowrap text-muted">
                    {{ purchase.priceLabel(quote) }}
                  </span>
                  <span
                    v-if="covered === quote.currency"
                    class="text-primary mt-1 inline-flex items-center gap-1 text-[11px] font-medium"
                  >
                    <UIcon
                      name="i-lucide-zap"
                      class="size-3"
                    />
                    {{ $t('subscription.allowanceBadge') }}
                  </span>
                </span>
              </label>
            </div>
          </fieldset>

          <UButton
            :label="buyLabel"
            size="lg"
            block
            :icon="covered === selected ? 'i-lucide-zap' : 'i-lucide-credit-card'"
            :loading="purchase.paying.value !== ''"
            :disabled="!selectedQuote || purchase.paying.value !== ''"
            @click="buy"
          />

          <p
            v-if="covered && covered === selected"
            class="text-xs text-muted"
          >
            {{ $t('subscription.fromAllowance') }}
          </p>

          <p class="text-xs text-muted">
            {{ $t('subscription.rateNote', { date: new Date(purchase.status.value.rated_at_unix * 1000).toLocaleString() }) }}
          </p>
        </template>
      </template>
    </div>
  </UCard>
</template>
