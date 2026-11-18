<script setup lang="ts">
import type { SupportedCurrency } from '~/composables/usePaymentApi'
import type { BudgetOpenCost } from '~/composables/useBudgetQuote'

/**
 * The first setup: a wallet is connected, but this currency has no allowance
 * yet. It is the same limit field as an existing allowance, plus the one
 * approval that creates it — the text explains what that approval does and why
 * the limit should start low.
 */
const amount = defineModel<string>({ required: true })

defineProps<{
  currency: SupportedCurrency
  placeholder: string
  cost: BudgetOpenCost | null
  costMissing: boolean
  /** A signature is already in flight. */
  busy: boolean
}>()

const emit = defineEmits<{ submit: [] }>()
</script>

<template>
  <BudgetLimitForm
    v-model="amount"
    :currency="currency"
    :placeholder="placeholder"
    :cost="cost"
    :cost-missing="costMissing"
  />

  <UButton
    :label="$t('budget.enable')"
    icon="i-lucide-wallet"
    :loading="busy"
    @click="emit('submit')"
  />

  <p class="text-sm text-muted">
    <span class="font-medium text-highlighted">{{ $t('budget.oneApproval.title') }}</span>
    {{ $t('budget.oneApproval.body') }}
  </p>

  <p class="text-sm text-muted">
    {{ $t('budget.startSmall') }}
    <NuxtLink
      to="/learn/two-wallets"
      class="font-medium text-primary underline-offset-4 hover:underline"
    >
      {{ $t('budget.keepSmall.twoWallets') }}
    </NuxtLink>.
  </p>
</template>
