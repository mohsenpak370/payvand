<script setup lang="ts">
import { currencySymbol, type SupportedCurrency } from '~/composables/usePaymentApi'
import type { BudgetOpenCost } from '~/composables/useBudgetQuote'

/**
 * The limit being set and what applying it costs.
 *
 * Both branches of the card use it — an existing allowance can be raised or
 * lowered, and a buyer with no allowance sets the first one — so the warning
 * about how much a lost device could spend stays beside the field in both.
 */
const amount = defineModel<string>({ required: true })

withDefaults(
  defineProps<{
    currency: SupportedCurrency
    placeholder: string
    /**
     * The save button can top the pot up, so the help says the limit applies on
     * the next signature. A first setup is that signature, so it adds nothing.
     */
    help?: boolean
    cost: BudgetOpenCost | null
    /** The chain could not price a real limit: say so instead of promising one. */
    costMissing: boolean
  }>(),
  { help: false }
)
</script>

<template>
  <UFormField
    :label="$t('budget.limitLabel', { currency: currencySymbol(currency) })"
    :help="help ? $t('budget.limitHelp') : undefined"
  >
    <UInput
      v-model="amount"
      type="number"
      min="0"
      step="0.1"
      class="w-full"
      :placeholder="placeholder"
    />
  </UFormField>

  <BudgetOpenCost :cost="cost" />
  <p
    v-if="costMissing"
    class="text-xs text-muted"
  >
    {{ $t('budget.cost.unavailable') }}
  </p>

  <UAlert
    color="warning"
    variant="subtle"
    icon="i-lucide-shield-alert"
    :title="$t('budget.lowLimitTitle')"
    :description="$t('budget.lowLimitWarning')"
  />
</template>
