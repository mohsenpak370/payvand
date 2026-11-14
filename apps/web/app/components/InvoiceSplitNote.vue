<script setup lang="ts">
import type { InvoiceCosts } from '~/utils/invoiceCost'
import { currencySymbol, formatTokenAmount } from '~/composables/usePaymentApi'

/**
 * What a code pays its seller, in the two facts a seller needs before sharing
 * it: the price minus the fee, and the rent that comes back on payment. The
 * numbers come from `useInvoiceCosts`, so both follow the policy on chain.
 */
const props = defineProps<{ costs: InvoiceCosts | null }>()

const netLabel = computed(() => {
  const costs = props.costs
  if (!costs) return ''
  return costs.currency === 'sol'
    ? `${lamportsToSol(costs.sellerReceives)} SOL`
    : `${formatTokenAmount(costs.sellerReceives, 6)} ${currencySymbol(costs.currency)}`
})
</script>

<template>
  <div
    v-if="costs"
    class="flex flex-col gap-1 text-xs text-muted"
  >
    <p>
      {{ $t('invoiceSplit.netReceive', { net: netLabel, percent: costs.feeBps / 100 }) }}
    </p>
    <p>
      {{ $t('invoiceSplit.rentNote', { rent: lamportsToSol(costs.invoiceRent) }) }}
    </p>
  </div>
</template>
