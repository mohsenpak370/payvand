<script setup lang="ts">
/** The title, one line of context and a refresh button every admin card shares. */
defineProps<{
  title: string
  subtitle: string
  loading: boolean
}>()

const emit = defineEmits<{ refresh: [] }>()
</script>

<template>
  <div class="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between sm:gap-3">
    <div>
      <h2 class="text-base font-semibold whitespace-nowrap">
        {{ title }}
      </h2>
      <p class="text-xs text-muted">
        {{ subtitle }}
      </p>
    </div>
    <UButton
      icon="i-lucide-refresh-cw"
      :label="$t('profile.refresh')"
      color="neutral"
      variant="soft"
      size="sm"
      :loading="loading"
      @click="emit('refresh')"
    />
  </div>
</template>
