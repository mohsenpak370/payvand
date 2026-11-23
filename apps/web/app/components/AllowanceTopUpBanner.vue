<script setup lang="ts">
/**
 * The top-up nudge, shared by the profile and budget pages.
 *
 * A month resets what an allowance can spend, not what it holds: the ceiling is
 * whole again on the 1st while the pot still has whatever was left. This says so
 * and offers the one approval that puts the money back — it is a decision here,
 * because the program cannot take money from the wallet on its own.
 */
defineProps<{
  /** The month turned over under this pot, so the reset explains the gap. */
  rolledOver: boolean
  /** The affected currencies, already written as symbols ("SOL, USDC"). */
  currencies: string
  /** What the button says; the pages word it for their own context. */
  actionLabel: string
}>()

defineEmits<{ topUp: [] }>()
</script>

<template>
  <UAlert
    color="warning"
    variant="subtle"
    icon="i-lucide-piggy-bank"
    :title="rolledOver ? $t('allowance.topUpTitle') : $t('allowance.lowPotTitle')"
    :description="$t('allowance.topUpBody', { currencies })"
  >
    <template #actions>
      <UButton
        :label="actionLabel"
        size="sm"
        @click="$emit('topUp')"
      />
    </template>
  </UAlert>
</template>
