<script setup lang="ts">
import type { PaymentsPage } from '~/composables/useAdminApi'
import { shorten } from '~/utils/profileFormat'
import { adminDateTime } from '~/utils/adminFormat'

/** The on-chain invoice feed. Its RPC read can fail without touching SQL data. */
defineProps<{
  feed: PaymentsPage | null
  loading: boolean
  failure: string
}>()

const emit = defineEmits<{ refresh: [] }>()

const { t } = useI18n()

function statusColor(status: string): 'success' | 'warning' | 'neutral' {
  return status === 'settled'
    ? 'success'
    : status === 'pending' || status === 'missing'
      ? 'warning'
      : 'neutral'
}

/** "1 invoice" / "2 invoices", in whatever forms the locale defines. */
function invoiceCount(count: number, cluster: string) {
  return t('admin.feed.count', { count, cluster: networkLabel(cluster) }, count)
}
</script>

<template>
  <UCard class="min-w-0">
    <template #header>
      <div class="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between sm:gap-3">
        <div>
          <h2 class="text-base font-semibold whitespace-nowrap">
            {{ $t('admin.feed.title') }}
          </h2>
          <p class="text-xs text-muted">
            {{
              feed
                ? invoiceCount(feed.total, feed.cluster)
                : $t('admin.feed.fallback')
            }}
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

    <UAlert
      v-if="failure"
      color="error"
      variant="subtle"
      icon="i-lucide-circle-alert"
      :title="$t('admin.feed.errorTitle')"
      :description="failure"
    />
    <p
      v-else-if="loading && !feed"
      class="py-6 text-center text-sm text-muted"
    >
      {{ $t('admin.feed.reading') }}
    </p>
    <p
      v-else-if="feed && feed.items.length === 0"
      class="py-6 text-center text-sm text-muted"
    >
      {{ $t('admin.feed.empty') }}
    </p>
    <!--
      Six columns cannot fit a phone, so the table keeps a readable width and
      scrolls sideways inside the card instead of stretching the page.
      `overflow-x-auto` also stops the table's minimum width from forcing every
      ancestor wider than the viewport.
    -->
    <div
      v-else-if="feed"
      class="overflow-x-auto"
    >
      <table class="w-full min-w-[44rem] text-sm">
        <thead>
          <tr class="border-b text-start text-muted">
            <th class="py-2 pe-3 font-medium">
              {{ $t('admin.feed.when') }}
            </th>
            <th class="py-2 pe-3 font-medium">
              {{ $t('admin.feed.seller') }}
            </th>
            <th class="py-2 pe-3 text-right font-medium">
              {{ $t('admin.feed.amount') }}
            </th>
            <th class="py-2 pe-3 font-medium">
              {{ $t('admin.feed.memo') }}
            </th>
            <th class="py-2 pe-3 font-medium">
              {{ $t('admin.feed.status') }}
            </th>
            <th class="py-2 font-medium">
              {{ $t('admin.feed.paidBy') }}
            </th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="payment in feed.items"
            :key="payment.id"
            class="border-b last:border-0"
          >
            <td class="py-2 pe-3 whitespace-nowrap text-muted">
              {{ adminDateTime(payment.settled_at_unix || payment.created_at_unix) }}
            </td>
            <td class="py-2 pe-3 font-mono text-xs">
              {{ shorten(payment.seller) }}
            </td>
            <td class="py-2 pe-3 text-right">
              {{ lamportsToSol(payment.amount_lamports) }}
            </td>
            <td class="max-w-[12rem] truncate py-2 pe-3">
              {{ payment.memo || '—' }}
            </td>
            <td class="py-2 pe-3">
              <UBadge
                :color="statusColor(payment.status)"
                variant="subtle"
              >
                {{ statusLabel(payment.status) }}
              </UBadge>
            </td>
            <td class="py-2 font-mono text-xs">
              {{ payment.settled_by ? shorten(payment.settled_by) : '—' }}
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </UCard>
</template>
