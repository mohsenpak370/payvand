<script setup lang="ts">
import type { WaitlistEntry, WaitlistPage } from '~/composables/useAdminApi'
import { adminDate } from '~/utils/adminFormat'

/** The waiting list: its own read, its own errors, its own removal. */
defineProps<{
  waitlist: WaitlistPage | null
  loading: boolean
  failure: string
  busy: boolean
}>()

const emit = defineEmits<{
  refresh: []
  remove: [entry: WaitlistEntry]
}>()

const { t } = useI18n()

/** "1 address" / "2 addresses", in whatever forms the locale defines. */
function addressCount(count: number) {
  return t('admin.waitlist.count', { count }, count)
}
</script>

<template>
  <UCard class="min-w-0">
    <template #header>
      <AdminCardHeader
        :title="$t('admin.waitlist.title')"
        :subtitle="waitlist ? addressCount(waitlist.total) : $t('admin.waitlist.fallback')"
        :loading="loading"
        @refresh="emit('refresh')"
      />
    </template>

    <UAlert
      v-if="failure"
      color="error"
      variant="subtle"
      icon="i-lucide-circle-alert"
      :title="$t('admin.waitlist.errorTitle')"
      :description="failure"
    />
    <p
      v-else-if="loading && !waitlist"
      class="py-6 text-center text-sm text-muted"
    >
      {{ $t('admin.waitlist.reading') }}
    </p>
    <p
      v-else-if="!waitlist?.items.length"
      class="py-6 text-center text-sm text-muted"
    >
      {{ $t('admin.waitlist.empty') }}
    </p>
    <ul
      v-else
      class="flex flex-col divide-y divide-default"
    >
      <li
        v-for="entry in waitlist.items"
        :key="entry.email"
        class="flex min-w-0 items-center justify-between gap-3 py-2.5"
      >
        <div class="min-w-0">
          <p class="text-sm break-all">
            {{ entry.email }}
          </p>
          <p class="text-xs text-muted">
            {{ $t('admin.waitlist.joined', { date: adminDate(entry.created_at_unix) }) }}
          </p>
        </div>
        <UButton
          icon="i-lucide-trash-2"
          color="error"
          variant="ghost"
          size="sm"
          :disabled="busy"
          :aria-label="$t('admin.waitlist.removeAria')"
          @click="emit('remove', entry)"
        />
      </li>
    </ul>
  </UCard>
</template>
