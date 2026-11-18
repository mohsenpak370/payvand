<script setup lang="ts">
import type { AdminStats } from '~/composables/useAdminApi'

defineProps<{ stats: AdminStats | null }>()
</script>

<template>
  <div class="grid grid-cols-2 gap-4 md:grid-cols-4">
    <UCard>
      <div class="flex flex-col gap-1">
        <span class="text-2xl font-semibold">{{ stats?.users ?? '—' }}</span>
        <span class="text-sm text-muted">{{ $t('admin.stats.accounts') }}</span>
      </div>
    </UCard>
    <UCard>
      <div class="flex flex-col gap-1">
        <span class="text-2xl font-semibold">{{ stats?.active_sessions ?? '—' }}</span>
        <span class="text-sm text-muted">{{ $t('admin.stats.sessions') }}</span>
      </div>
    </UCard>
    <UCard>
      <div class="flex flex-col gap-1">
        <span class="text-2xl font-semibold">{{ stats?.discount_codes ?? '—' }}</span>
        <span class="text-sm text-muted">{{ $t('admin.stats.discounts') }}</span>
      </div>
    </UCard>
    <UCard>
      <div class="flex flex-col gap-1">
        <span class="text-2xl font-semibold">{{ stats?.pending_resets ?? '—' }}</span>
        <span class="text-sm text-muted">{{ $t('admin.stats.resets') }}</span>
      </div>
    </UCard>
  </div>
</template>
