<script setup lang="ts">
import {
  MAX_SOL_FEE_LAMPORTS,
  MAX_TOKEN_FEE_BASE_UNITS,
  type InvoiceCosts
} from '~/utils/invoiceCost'
import { currencySymbol, formatTokenAmount } from '~/composables/usePaymentApi'

/**
 * What a code pays its seller, on one line: the price minus the fee. The info
 * button opens the detail — the percentage, the flat for the permanent payment
 * record, and where the rent goes — because a seller wants the net at a glance
 * and the arithmetic only when they ask. Every number comes from
 * `useInvoiceCosts`, so both follow the policy on chain.
 */
const props = defineProps<{ costs: InvoiceCosts | null }>()

function money(amount: number, currency: InvoiceCosts['currency']): string {
  return currency === 'sol'
    ? `${lamportsToSol(amount)} SOL`
    : `${formatTokenAmount(amount, 6)} ${currencySymbol(currency)}`
}

const labels = computed(() => {
  const costs = props.costs
  if (!costs) return { net: '', fee: '', record: '', max: '' }
  return {
    net: money(costs.sellerReceives, costs.currency),
    fee: money(costs.fee, costs.currency),
    record: money(costs.recordFee, costs.currency),
    // The ceiling in the invoice's own currency, so the line can say what the
    // maximum is rather than only that a maximum exists.
    max: money(
      costs.currency === 'sol' ? MAX_SOL_FEE_LAMPORTS : MAX_TOKEN_FEE_BASE_UNITS,
      costs.currency
    )
  }
})
</script>

<template>
  <div
    v-if="costs"
    class="flex items-center gap-1 text-xs text-muted"
  >
    <span>{{ $t('invoiceSplit.netReceive', { net: labels.net }) }}</span>
    <UPopover :ui="{ content: 'p-3 text-xs leading-relaxed max-w-xs' }">
      <UButton
        icon="i-lucide-info"
        color="neutral"
        variant="ghost"
        size="xs"
        class="-my-1"
        :aria-label="$t('invoiceSplit.detailsAria')"
      />
      <template #content>
        <div class="space-y-2">
          <p v-if="costs.feeIsFloor">
            {{ costs.currency === 'sol'
              ? $t('invoiceSplit.feeNoneLine')
              : $t('invoiceSplit.feeFloorLine', { fee: labels.fee }) }}
          </p>
          <p v-else-if="costs.feeIsCapped">
            {{ $t('invoiceSplit.feeCapLine', {
              percent: costs.feeBps / 100,
              fee: labels.fee,
              max: labels.max
            }) }}
          </p>
          <p v-else>
            {{ $t('invoiceSplit.feeLine', { percent: costs.feeBps / 100, fee: labels.fee }) }}
          </p>
          <p v-if="costs.recordFee > 0 && !costs.feeIsFloor">
            {{ $t('invoiceSplit.recordLine', { fee: labels.record }) }}
          </p>
          <p>
            {{ costs.currency === 'sol'
              ? $t('invoiceSplit.rentNote', { rent: lamportsToSol(costs.invoiceRent) })
              : $t('invoiceSplit.rentNoteToken') }}
          </p>
          <NuxtLink
            to="/learn/what-is-a-fee"
            class="inline-block underline underline-offset-2 hover:text-default transition-colors"
          >
            {{ $t('invoiceSplit.learnMore') }}
          </NuxtLink>
        </div>
      </template>
    </UPopover>
  </div>
</template>
