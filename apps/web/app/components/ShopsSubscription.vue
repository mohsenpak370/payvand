<script setup lang="ts">
// The shops feature is sold by the month: €1.70 for 30 days, in SOL, USDC or
// EURC. The purchase lives in `useSubscriptionPurchase` because the New shop
// dialog offers the same one; this card is its full-page face.
const purchase = useSubscriptionPurchase()

const expiry = computed(() => {
  const seconds = purchase.status.value?.expires_at_unix ?? 0
  if (!seconds) return ''
  return new Date(seconds * 1000).toLocaleDateString()
})

onMounted(() => {
  void purchase.load()
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
          v-if="purchase.status.value"
          :color="purchase.status.value.active ? 'success' : 'neutral'"
          variant="subtle"
          class="shrink-0"
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
        <p
          v-if="purchase.status.value.active"
          class="text-sm text-muted"
        >
          {{ $t('subscription.renewsOn', { date: expiry }) }}
        </p>
        <p
          v-else
          class="text-sm text-muted"
        >
          {{ $t('subscription.price', { price: '€1.70', days: purchase.status.value.period_days }) }}
        </p>

        <!-- No rate, no price: the status above still stands, and only these
             buttons are unavailable until the next read answers. -->
        <p
          v-if="!purchase.status.value.rates_available"
          class="text-sm text-muted"
        >
          {{ $t('subscription.ratesUnavailable') }}
        </p>

        <div
          v-if="purchase.status.value.rates_available"
          class="flex flex-wrap gap-2"
        >
          <UButton
            v-for="quote in purchase.quotes.value"
            :key="quote.currency"
            :label="purchase.priceLabel(quote)"
            :loading="purchase.paying.value === quote.currency"
            :disabled="purchase.paying.value !== ''"
            color="neutral"
            variant="soft"
            size="sm"
            :icon="purchase.covered.value === quote.currency ? 'i-lucide-zap' : 'i-lucide-credit-card'"
            @click="purchase.pay(quote.currency)"
          />
        </div>

        <p
          v-if="purchase.covered.value"
          class="text-xs text-muted"
        >
          {{ $t('subscription.fromAllowance') }}
        </p>

        <p
          v-if="purchase.status.value.rates_available"
          class="text-xs text-muted"
        >
          {{ $t('subscription.rateNote', { date: new Date(purchase.status.value.rated_at_unix * 1000).toLocaleString() }) }}
        </p>
      </template>
    </div>
  </UCard>
</template>
