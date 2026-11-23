<script setup lang="ts">
import type { MyTransaction } from '~/composables/useAuth'
import type { FiatCurrency } from '~/composables/useFiat'
import { FLOW_CURRENCIES, summarizeFlow, type CoinTotals, type FlowSide } from '~/utils/profileTotals'

/**
 * What came in, what went out, and the difference — in the money the person
 * reads prices in, with the coins behind each figure spelled out.
 *
 * The figures cover the payments this page loaded (the wallet's recent
 * history), which is what the card says rather than pretending to be a
 * lifetime statement.
 */
const props = defineProps<{
  items: MyTransaction[]
  displayCurrency: FiatCurrency
  loading: boolean
}>()

const { t } = useI18n()
const rates = useSolRates()

const flow = computed(() => summarizeFlow(props.items, rates.value, props.displayCurrency))

const sides = computed(() => [
  {
    key: 'spent',
    label: t('profile.flowSpent'),
    icon: 'i-lucide-arrow-up-right',
    tone: 'text-error',
    side: flow.value.spent
  },
  {
    key: 'received',
    label: t('profile.flowReceived'),
    icon: 'i-lucide-arrow-down-left',
    tone: 'text-success',
    side: flow.value.received
  },
  {
    key: 'net',
    label: t('profile.flowNet'),
    icon: 'i-lucide-scale',
    tone: 'text-highlighted',
    side: flow.value.net
  }
])

/** The coins with something in them: a zero line is noise in a total. */
function coinLines(coins: CoinTotals): { key: string, amount: string }[] {
  return FLOW_CURRENCIES.filter(currency => coins[currency] !== 0).map((currency) => {
    const amount = currency === 'sol'
      ? `${lamportsToSol(Math.abs(coins.sol))} SOL`
      : `${formatTokenAmount(Math.abs(coins[currency]), 6)} ${currencySymbol(currency)}`
    return { key: currency, amount: `${coins[currency] < 0 ? '−' : ''}${amount}` }
  })
}

function fiatLabel(side: FlowSide): string {
  return side.fiat === null ? '—' : formatFiat(side.fiat, props.displayCurrency)
}

/** True when nothing settled at all, so the card can say so once instead of three times. */
const empty = computed(() =>
  FLOW_CURRENCIES.every(currency => flow.value.spent.coins[currency] === 0
    && flow.value.received.coins[currency] === 0)
)
</script>

<template>
  <UCard class="min-w-0">
    <template #header>
      <div>
        <h2 class="text-base font-semibold">
          {{ $t('profile.flowTitle') }}
        </h2>
        <p class="text-sm text-muted">
          {{ $t('profile.flowDescription', { currency: displayCurrency.toUpperCase() }) }}
        </p>
      </div>
    </template>

    <p
      v-if="loading && !items.length"
      class="py-4 text-center text-sm text-muted"
    >
      {{ $t('profile.txLoading') }}
    </p>

    <p
      v-else-if="empty"
      class="py-4 text-center text-sm text-muted"
    >
      {{ $t('profile.flowEmpty') }}
    </p>

    <div
      v-else
      class="grid gap-4 sm:grid-cols-3"
    >
      <div
        v-for="side in sides"
        :key="side.key"
        class="flex min-w-0 flex-col gap-1 rounded-xl border border-default p-4"
      >
        <span class="text-muted flex items-center gap-1.5 text-xs font-medium">
          <UIcon
            :name="side.icon"
            class="size-4"
            :class="side.tone"
          />
          {{ side.label }}
        </span>
        <span
          class="text-xl font-semibold tabular-nums"
          :class="side.key === 'spent' ? 'text-error' : side.key === 'received' ? 'text-success' : ''"
        >
          {{ fiatLabel(side.side) }}
        </span>
        <span
          v-if="side.side.fiat === null"
          class="text-muted text-xs"
        >
          {{ $t('profile.flowNoRate') }}
        </span>

        <!-- Only the coins that moved: a total that lists three zeroes hides the
             one number that matters. -->
        <ul class="mt-1 flex flex-col gap-0.5">
          <li
            v-for="line in coinLines(side.side.coins)"
            :key="`${side.key}-${line.key}`"
            class="text-sm tabular-nums"
          >
            {{ line.amount }}
          </li>
        </ul>
      </div>
    </div>
  </UCard>
</template>
