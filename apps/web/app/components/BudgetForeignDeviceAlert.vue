<script setup lang="ts">
/**
 * Says the allowance on screen is registered to another device, and offers the
 * one approval that moves it here. It moves no funds and revokes the other
 * device, so the description names the key that would stop working.
 */
const props = defineProps<{
  /** This device's key, if it has one: the description names it. */
  deviceKeyAddress: string | null
  /** A signature is already in flight. */
  busy: boolean
}>()

const emit = defineEmits<{ takeOver: [] }>()

const { t } = useI18n()

const description = computed(() => {
  const key = props.deviceKeyAddress
  return key
    ? t('budget.foreign.withKey', { key: shorten(key) })
    : t('budget.foreign.description')
})

function shorten(value: string) {
  return `${value.slice(0, 4)}…${value.slice(-4)}`
}
</script>

<template>
  <UAlert
    color="info"
    variant="subtle"
    icon="i-lucide-smartphone"
    :title="$t('budget.foreign.title')"
    :description="description"
  >
    <template #actions>
      <UButton
        :label="$t('budget.foreign.action')"
        size="sm"
        :loading="busy"
        @click="emit('takeOver')"
      />
    </template>
  </UAlert>
</template>
