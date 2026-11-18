<script setup lang="ts">
/**
 * The float a session key still holds after it stopped paying for anything.
 * It is only ever shown when no allowance can spend it, so the button is the
 * one way back to the wallet — and it forgets the key once the sweep lands.
 */
defineProps<{
  /** Lamports stranded on this device's key. */
  amount: number
  /** A sweep is already in flight. */
  busy: boolean
}>()

const emit = defineEmits<{ recover: [] }>()
</script>

<template>
  <UAlert
    color="warning"
    variant="subtle"
    icon="i-lucide-hand-coins"
    :title="$t('budget.recover.title')"
    :description="$t('budget.recover.body', { amount: lamportsToSol(amount) })"
  >
    <template #actions>
      <UButton
        :label="$t('budget.recover.action', { amount: lamportsToSol(amount) })"
        size="sm"
        :loading="busy"
        @click="emit('recover')"
      />
    </template>
  </UAlert>
</template>
