<script setup lang="ts">
import type { AirdropPage } from '~/composables/useAdminApi'
import { adminDateTime } from '~/utils/adminFormat'
import { tokenUnits } from '~/utils/adminChain'

/** Every starter-token claim, newest first, with what the faucet sent in total. */
const props = defineProps<{
  airdrops: AirdropPage | null
  loading: boolean
  failure: string
}>()

const emit = defineEmits<{
  refresh: []
  previous: []
  next: []
}>()

const { t } = useI18n()

const pageFrom = computed(() => (props.airdrops?.total ? props.airdrops.offset + 1 : 0))
const pageTo = computed(() =>
  props.airdrops ? Math.min(props.airdrops.offset + props.airdrops.limit, props.airdrops.total) : 0
)

function subtitle(page: AirdropPage | null) {
  if (!page) return t('admin.airdrops.fallback')
  return t('admin.airdrops.summary', {
    count: page.total,
    sol: lamportsToSol(page.total_sol_lamports),
    tokens: tokenUnits(page.total_token_base_units)
  }, page.total)
}
</script>

<template>
  <UCard class="min-w-0">
    <template #header>
      <AdminCardHeader
        :title="$t('admin.airdrops.title')"
        :subtitle="subtitle(airdrops)"
        :loading="loading"
        @refresh="emit('refresh')"
      />
    </template>

    <UAlert
      v-if="failure"
      color="error"
      variant="subtle"
      icon="i-lucide-circle-alert"
      :title="$t('admin.airdrops.errorTitle')"
      :description="failure"
    />
    <p
      v-else-if="loading && !airdrops"
      class="py-6 text-center text-sm text-muted"
    >
      {{ $t('common.loading') }}
    </p>
    <p
      v-else-if="!airdrops?.items.length"
      class="py-6 text-center text-sm text-muted"
    >
      {{ $t('admin.airdrops.empty') }}
    </p>
    <template v-else>
      <div class="overflow-x-auto">
        <table class="w-full min-w-[44rem] text-sm">
          <thead>
            <tr class="border-b text-start text-muted">
              <th class="py-2 pe-3 font-medium">
                {{ $t('admin.feed.when') }}
              </th>
              <th class="py-2 pe-3 font-medium">
                {{ $t('admin.users.account') }}
              </th>
              <th class="py-2 pe-3 font-medium">
                {{ $t('admin.airdrops.wallet') }}
              </th>
              <th class="py-2 pe-3 text-right font-medium">
                {{ $t('admin.feed.amount') }}
              </th>
              <th class="py-2 font-medium">
                {{ $t('admin.airdrops.transaction') }}
              </th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="drop in airdrops.items"
              :key="drop.user_id"
              class="border-b last:border-0"
            >
              <td class="py-2 pe-3 whitespace-nowrap text-muted">
                {{ adminDateTime(drop.claimed_at) }}
              </td>
              <td class="max-w-[14rem] truncate py-2 pe-3">
                {{ drop.email || '—' }}
              </td>
              <td class="py-2 pe-3">
                <AdminAddress
                  :value="drop.wallet"
                  :cluster="drop.cluster"
                />
              </td>
              <td class="py-2 pe-3 text-right text-xs whitespace-nowrap">
                {{ lamportsToSol(drop.sol_lamports) }} SOL ·
                {{ tokenUnits(drop.token_base_units) }} USDC ·
                {{ tokenUnits(drop.token_base_units) }} EURC
              </td>
              <td class="py-2">
                <AdminAddress
                  v-if="drop.signature"
                  :value="drop.signature"
                  :cluster="drop.cluster"
                  kind="tx"
                />
                <UBadge
                  v-else
                  color="warning"
                  variant="subtle"
                >
                  {{ $t('admin.airdrops.pending') }}
                </UBadge>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <div
        v-if="airdrops.total > airdrops.limit"
        class="mt-4 flex flex-wrap items-center justify-between gap-3 text-sm text-muted"
      >
        <span>{{ $t('admin.airdrops.range', { from: pageFrom, to: pageTo, total: airdrops.total }) }}</span>
        <div class="flex gap-2">
          <UButton
            :label="$t('admin.users.previous')"
            color="neutral"
            variant="soft"
            size="sm"
            :disabled="airdrops.offset === 0 || loading"
            @click="emit('previous')"
          />
          <UButton
            :label="$t('admin.users.next')"
            color="neutral"
            variant="soft"
            size="sm"
            :disabled="pageTo >= airdrops.total || loading"
            @click="emit('next')"
          />
        </div>
      </div>
    </template>
  </UCard>
</template>
