<script setup lang="ts">
/** The edit and delete buttons at the end of a record row. */
defineProps<{
  busy: boolean
  /** Codes have no edit: a changed code is a different code. */
  editable?: boolean
}>()

const emit = defineEmits<{ edit: [], remove: [] }>()
</script>

<template>
  <div class="inline-flex gap-1">
    <UButton
      v-if="editable"
      icon="i-lucide-pencil"
      color="neutral"
      variant="ghost"
      size="sm"
      :disabled="busy"
      :aria-label="$t('admin.records.edit')"
      :title="$t('admin.records.edit')"
      @click="emit('edit')"
    />
    <UButton
      icon="i-lucide-trash-2"
      color="error"
      variant="ghost"
      size="sm"
      :disabled="busy"
      :aria-label="$t('admin.users.delete')"
      :title="$t('admin.users.delete')"
      @click="emit('remove')"
    />
  </div>
</template>
