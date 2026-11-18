<script setup lang="ts">
import type { SupportedCurrency } from '~/composables/usePaymentApi'

/**
 * One allowance per currency: SOL holds lamports, each stablecoin holds its own
 * tokens. This switch decides which allowance the card below is showing.
 */
const currency = defineModel<SupportedCurrency>({ required: true })

defineProps<{ currencies: SupportedCurrency[] }>()
</script>

<template>
  <div class="flex flex-wrap gap-2">
    <UButton
      v-for="choice in currencies"
      :key="choice"
      :label="currencySymbol(choice)"
      size="sm"
      :color="currency === choice ? 'primary' : 'neutral'"
      :variant="currency === choice ? 'solid' : 'outline'"
      @click="currency = choice"
    />
  </div>
</template>
