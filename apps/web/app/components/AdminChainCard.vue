<script setup lang="ts">
import type { ChainAccount, ChainOverview } from '~/composables/useAdminApi'
import { adminDateTime } from '~/utils/adminFormat'
import { rentSurplus, tokenUnits } from '~/utils/adminChain'

/**
 * The program and the accounts it owns: where each lives, how big it is, what
 * it holds, and the rent-exempt minimum the cluster charges for that size.
 */
defineProps<{
  chain: ChainOverview | null
  loading: boolean
  failure: string
}>()

const emit = defineEmits<{ refresh: [] }>()

const { t } = useI18n()

function rentState(account: ChainAccount): { color: 'success' | 'warning' | 'error', label: string } {
  if (!account.exists) return { color: 'error', label: t('admin.chain.missing') }
  if (account.lamports >= account.rent_exempt_lamports) {
    return { color: 'success', label: t('admin.chain.rentExempt') }
  }
  return { color: 'warning', label: t('admin.chain.belowRent') }
}

function subtitle(chain: ChainOverview | null) {
  if (!chain) return t('admin.chain.fallback')
  return t('admin.chain.readAt', {
    cluster: networkLabel(chain.cluster),
    time: adminDateTime(chain.read_at)
  })
}
</script>

<template>
  <UCard class="min-w-0">
    <template #header>
      <AdminCardHeader
        :title="$t('admin.chain.title')"
        :subtitle="subtitle(chain)"
        :loading="loading"
        @refresh="emit('refresh')"
      />
    </template>

    <UAlert
      v-if="failure"
      color="error"
      variant="subtle"
      icon="i-lucide-circle-alert"
      :title="$t('admin.chain.errorTitle')"
      :description="failure"
    />
    <p
      v-else-if="loading && !chain"
      class="py-6 text-center text-sm text-muted"
    >
      {{ $t('admin.feed.reading') }}
    </p>
    <div
      v-else-if="chain"
      class="flex flex-col gap-4"
    >
      <dl class="grid grid-cols-1 gap-3 text-sm sm:grid-cols-2">
        <div>
          <dt class="text-xs text-muted">
            {{ $t('admin.chain.upgradeAuthority') }}
          </dt>
          <dd>
            <AdminAddress
              v-if="chain.program.upgrade_authority"
              :value="chain.program.upgrade_authority"
              :cluster="chain.cluster"
            />
            <span v-else>{{ $t('admin.chain.immutable') }}</span>
          </dd>
        </div>
        <div>
          <dt class="text-xs text-muted">
            {{ $t('admin.chain.lastDeploy') }}
          </dt>
          <dd class="font-mono text-xs leading-7">
            {{ chain.program.last_deploy_slot?.toLocaleString() ?? '—' }}
          </dd>
        </div>
      </dl>

      <div class="overflow-x-auto">
        <table class="w-full min-w-[44rem] text-sm">
          <thead>
            <tr class="border-b text-start text-muted">
              <th class="py-2 pe-3 font-medium">
                {{ $t('admin.chain.account') }}
              </th>
              <th class="py-2 pe-3 font-medium">
                {{ $t('admin.chain.address') }}
              </th>
              <th class="py-2 pe-3 text-right font-medium">
                {{ $t('admin.chain.size') }}
              </th>
              <th class="py-2 pe-3 text-right font-medium">
                {{ $t('admin.chain.balance') }}
              </th>
              <th class="py-2 pe-3 text-right font-medium">
                {{ $t('admin.chain.rent') }}
              </th>
              <th class="py-2 font-medium">
                {{ $t('admin.chain.status') }}
              </th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="account in chain.accounts"
              :key="account.key"
              class="border-b align-top last:border-0"
            >
              <td class="py-2 pe-3">
                <p class="font-medium">
                  {{ $t(`admin.chain.names.${account.key}`) }}
                </p>
                <p class="max-w-[16rem] text-xs text-muted">
                  {{ $t(`admin.chain.about.${account.key}`) }}
                </p>
              </td>
              <td class="py-2 pe-3">
                <AdminAddress
                  :value="account.address"
                  :cluster="chain.cluster"
                />
              </td>
              <td class="py-2 pe-3 text-right whitespace-nowrap">
                {{ $t('admin.chain.bytes', { count: account.data_len.toLocaleString() }) }}
              </td>
              <td class="py-2 pe-3 text-right whitespace-nowrap">
                <p>{{ lamportsToSol(account.lamports) }} SOL</p>
                <template v-if="account.tokens">
                  <p class="text-xs text-muted">
                    {{ account.tokens.usdc_base_units === null ? '—' : tokenUnits(account.tokens.usdc_base_units) }} USDC
                  </p>
                  <p class="text-xs text-muted">
                    {{ account.tokens.eurc_base_units === null ? '—' : tokenUnits(account.tokens.eurc_base_units) }} EURC
                  </p>
                </template>
              </td>
              <td class="py-2 pe-3 text-right whitespace-nowrap">
                <p>{{ lamportsToSol(account.rent_exempt_lamports) }} SOL</p>
                <p
                  v-if="account.key === 'treasury'"
                  class="text-xs text-muted"
                >
                  {{ $t('admin.chain.aboveRent', { amount: lamportsToSol(rentSurplus(account)) }) }}
                </p>
              </td>
              <td class="py-2">
                <UBadge
                  :color="rentState(account).color"
                  variant="subtle"
                >
                  {{ rentState(account).label }}
                </UBadge>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </UCard>
</template>
