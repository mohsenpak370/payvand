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
    <!--
      Only a SOL invoice pays its rent to the seller (the account closes to
      them). A token invoice refunds the payer's rent from the treasury and
      closes to the treasury, so the seller gets no rent back and needs no SOL
      to hold the code — hence two sentences rather than one.
    -->
    <p>
      {{ costs.currency === 'sol'
        ? $t('invoiceSplit.rentNote', { rent: lamportsToSol(costs.invoiceRent) })
        : $t('invoiceSplit.rentNoteToken') }}
    </p>
  </div>
</template>
