<script setup lang="ts">
import { currencySymbol, type SupportedCurrency } from '~/composables/usePaymentApi'

/**
 * What can be done with the allowance on screen: put the pot back to the
 * month's limit, save a new limit, or close it and reclaim the funds. The
 * buttons own nothing but the press — the amounts come from the allowance the
 * page is showing.
 */
const props = defineProps<{
  currency: SupportedCurrency
  /** The month's ceiling, in base units: what a refill returns the pot to. */
  limit: number
  /** What a refill would add; 0 hides the button. */
  refillNeeded: number
  /** A signature is already in flight, so every one of these waits. */
  busy: boolean
}>()

const emit = defineEmits<{ refill: [], save: [], close: [] }>()

/** `Refill to 0.1 SOL` — the sum this refill would put the pot back to. */
const refillLabel = computed(() => budgetAmountLabel(props.currency, props.limit))
</script>

<template>
  <div class="flex flex-wrap gap-2">
    <UButton
      v-if="refillNeeded > 0"
      :label="$t('budget.refill', { amount: refillLabel })"
      icon="i-lucide-piggy-bank"
      :loading="busy"
      @click="emit('refill')"
    />
    <UButton
      :label="$t('budget.saveLimit')"
      icon="i-lucide-refresh-cw"
      :loading="busy"
      @click="emit('save')"
    />
    <UButton
      :label="$t('budget.closeBudget', { currency: currencySymbol(currency) })"
      color="error"
      variant="soft"
      icon="i-lucide-trash-2"
      :loading="busy"
      @click="emit('close')"
    />
  </div>
</template>
