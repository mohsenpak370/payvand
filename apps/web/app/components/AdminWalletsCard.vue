<script setup lang="ts">
import type { ChainOverview, ServerWallet } from '~/composables/useAdminApi'
import { claimsLeft, LOW_CLAIMS, LOW_SETTLES, settlesLeft, tokenUnits } from '~/utils/adminChain'

/**
 * The keys this server spends from — the faucet that sends starter tokens and
 * the relayer that sponsors settle fees — with their SOL, USDC and EURC, and
 * how many more claims or settles that pays for.
 */
const props = withDefaults(
  defineProps<{
    chain: ChainOverview | null
    loading: boolean
    failure: string
    /** Show only these wallets; all of them when omitted. */
    only?: ServerWallet['key'][]
  }>(),
  { only: undefined }
)

const emit = defineEmits<{ refresh: [] }>()

const { t } = useI18n()

const wallets = computed(() =>
  (props.chain?.wallets ?? []).filter(wallet => !props.only || props.only.includes(wallet.key))
)

interface Runway {
  count: number
  low: boolean
  label: string
}

function runway(wallet: ServerWallet): Runway | null {
  const chain = props.chain
  if (!chain || !wallet.account) return null
  if (wallet.key === 'faucet') {
    const count = claimsLeft(chain)
    if (count === null) return null
    return { count, low: count < LOW_CLAIMS, label: t('admin.wallets.claimsLeft', { count }, count) }
  }
  const count = settlesLeft(chain)
  if (count === null) return null
  return { count, low: count < LOW_SETTLES, label: t('admin.wallets.settlesLeft', { count }, count) }
}

function token(value: number | null | undefined) {
  return value === null || value === undefined ? '—' : tokenUnits(value)
}
</script>

<template>
  <UCard class="min-w-0">
    <template #header>
      <AdminCardHeader
        :title="$t('admin.wallets.title')"
        :subtitle="chain ? networkLabel(chain.cluster) : $t('admin.chain.fallback')"
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
    <ul
      v-else-if="chain"
      class="flex flex-col divide-y divide-default"
    >
      <li
        v-for="wallet in wallets"
        :key="wallet.key"
        class="flex flex-col gap-3 py-4 first:pt-0 last:pb-0"
      >
        <div class="flex flex-wrap items-start justify-between gap-2">
          <div class="min-w-0">
            <p class="font-medium">
              {{ $t(`admin.chain.names.${wallet.key}`) }}
            </p>
            <p class="text-xs text-muted">
              {{ $t(`admin.chain.about.${wallet.key}`) }}
            </p>
          </div>
          <AdminAddress
            v-if="wallet.account"
            :value="wallet.account.address"
            :cluster="chain.cluster"
          />
        </div>

        <UAlert
          v-if="!wallet.configured"
          color="neutral"
          variant="subtle"
          icon="i-lucide-key-round"
          :description="$t(`admin.wallets.unset.${wallet.key}`)"
        />
        <template v-else-if="wallet.account">
          <div class="grid grid-cols-3 gap-3">
            <div class="rounded-md bg-elevated px-3 py-2">
              <p class="text-lg font-semibold">
                {{ lamportsToSol(wallet.account.lamports) }}
              </p>
              <p class="text-xs text-muted">
                SOL
              </p>
            </div>
            <div class="rounded-md bg-elevated px-3 py-2">
              <p class="text-lg font-semibold">
                {{ token(wallet.account.tokens?.usdc_base_units) }}
              </p>
              <p class="text-xs text-muted">
                USDC
              </p>
            </div>
            <div class="rounded-md bg-elevated px-3 py-2">
              <p class="text-lg font-semibold">
                {{ token(wallet.account.tokens?.eurc_base_units) }}
              </p>
              <p class="text-xs text-muted">
                EURC
              </p>
            </div>
          </div>
          <div
            v-if="runway(wallet)"
            class="flex flex-wrap items-center gap-2 text-sm"
          >
            <UBadge
              :color="runway(wallet)!.low ? 'warning' : 'success'"
              variant="subtle"
              :icon="runway(wallet)!.low ? 'i-lucide-triangle-alert' : 'i-lucide-circle-check'"
            >
              {{ runway(wallet)!.label }}
            </UBadge>
            <span
              v-if="wallet.key === 'faucet'"
              class="text-xs text-muted"
            >
              {{
                $t('admin.wallets.perClaim', {
                  sol: lamportsToSol(chain.faucet_sol_lamports),
                  tokens: tokenUnits(chain.faucet_token_base_units)
                })
              }}
            </span>
            <span
              v-else
              class="text-xs text-muted"
            >
              {{ $t('admin.wallets.perSettle', { sol: lamportsToSol(chain.sponsor_settle_lamports) }) }}
            </span>
          </div>
          <p
            v-if="wallet.key === 'faucet' && !chain.faucet_enabled"
            class="text-xs text-muted"
          >
            {{ $t('admin.wallets.faucetOff') }}
          </p>
        </template>
      </li>
    </ul>
  </UCard>
</template>
