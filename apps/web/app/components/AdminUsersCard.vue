<script setup lang="ts">
import type { AdminUser } from '~/composables/useAdminApi'

/**
 * Account search, results and paging. The search draft is local to the form;
 * loading, rows and paging come from useAdminUsers through the page.
 */
const props = defineProps<{
  rows: AdminUser[]
  total: number
  pageSize: number
  offset: number
  loading: boolean
  failure: string
  busy: boolean
}>()

const emit = defineEmits<{
  search: [query: string]
  delete: [row: AdminUser]
  previous: []
  next: []
}>()

const query = ref('')

const pageFrom = computed(() => (props.total === 0 ? 0 : props.offset + 1))
const pageTo = computed(() => Math.min(props.offset + props.pageSize, props.total))

function submit() {
  emit('search', query.value)
}
</script>

<template>
  <UCard class="min-w-0">
    <form
      class="mb-4 flex gap-2"
      @submit.prevent="submit"
    >
      <UInput
        v-model="query"
        class="min-w-0 flex-1"
        :placeholder="$t('admin.users.searchPlaceholder')"
        icon="i-lucide-search"
      />
      <UButton
        type="submit"
        :label="$t('admin.users.search')"
        :loading="loading"
      />
    </form>

    <UAlert
      v-if="failure"
      color="error"
      variant="subtle"
      icon="i-lucide-circle-alert"
      :title="$t('admin.users.errorTitle')"
      :description="failure"
    />

    <p
      v-else-if="loading"
      class="py-6 text-center text-sm text-muted"
    >
      {{ $t('common.loading') }}
    </p>

    <p
      v-else-if="rows.length === 0"
      class="py-6 text-center text-sm text-muted"
    >
      {{ $t('admin.users.empty') }}
    </p>

    <AdminUsersTable
      v-else
      :rows="rows"
      :busy="busy"
      @delete="emit('delete', $event)"
    />

    <div
      v-if="total > pageSize"
      class="mt-4 flex flex-wrap items-center justify-between gap-3 text-sm text-muted"
    >
      <span>{{ $t('admin.users.range', { from: pageFrom, to: pageTo, total }) }}</span>
      <div class="flex gap-2">
        <UButton
          :label="$t('admin.users.previous')"
          color="neutral"
          variant="soft"
          size="sm"
          :disabled="offset === 0 || loading"
          @click="emit('previous')"
        />
        <UButton
          :label="$t('admin.users.next')"
          color="neutral"
          variant="soft"
          size="sm"
          :disabled="pageTo >= total || loading"
          @click="emit('next')"
        />
      </div>
    </div>
  </UCard>
</template>
