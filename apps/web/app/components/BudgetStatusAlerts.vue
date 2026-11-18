<script setup lang="ts">
/**
 * What the last attempt did: a refusal with its own title, or the line that
 * says a signature landed. Both are set by the action that ran, so a message
 * never outlives the attempt that produced it.
 */
defineProps<{
  failure: string
  failureTitle: string
  success: string
}>()
</script>

<template>
  <UAlert
    v-if="failure"
    color="error"
    variant="subtle"
    icon="i-lucide-circle-alert"
    :title="failureTitle"
    :description="failure"
  />
  <UAlert
    v-if="success"
    color="success"
    variant="subtle"
    icon="i-lucide-circle-check"
    :title="$t('common.done')"
    :description="success"
  />
</template>
