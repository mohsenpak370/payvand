import type { Address } from '@vue-solana/nuxt/kit'
import type { Ref } from 'vue'
import type { BudgetState, TokenBudgetState } from './budgetMath'
import type { SupportedCurrency } from './usePaymentApi'
import { potNeedsTopUp } from './budgetMath'

interface UseProfileAllowanceOptions {
  linkedWallet: Readonly<Ref<string | null>>
  publicKey: Readonly<Ref<Address | null>>
}

/** One currency's allowance as the profile card shows it. */
export interface ProfileAllowanceRow {
  currency: SupportedCurrency
  /** This month's ceiling, in the currency's base units. */
  limit: number
  /** What has been spent this month, in the same units; a new month starts at 0. */
  spent: number
  /** What the ceiling leaves, in the same units, before the pot is considered. */
  remaining: number
  /**
   * What a payment can actually draw on right now: the remaining ceiling,
   * capped by what the pot holds. The month resets the first number, not the
   * second, so this is the one the card shows as the headline.
   */
  spendable: number
  /** What the pot or vault holds right now, in the same units. */
  pot: number
  /** The pot is low enough that a top-up would change what can be paid. */
  needsTopUp: boolean
  /** The month rolled over under this allowance: the counter reset, no money moved. */
  rolledOver: boolean
  /** False when this wallet has no allowance in this currency at all. */
  set: boolean
}

/** One row from an allowance account, or the empty row for a currency without one. */
function rowFor(
  currency: SupportedCurrency,
  state: { monthlyLimit: number, spent: number, monthIndex: number } | null,
  pot: number,
  now: number
): ProfileAllowanceRow {
  if (!state) {
    return {
      currency,
      limit: 0,
      spent: 0,
      remaining: 0,
      spendable: 0,
      pot,
      needsTopUp: false,
      rolledOver: false,
      set: false
    }
  }
  const rolledOver = state.monthIndex !== now
  const spent = rolledOver ? 0 : state.spent
  const remaining = Math.max(0, state.monthlyLimit - spent)
  const spendable = Math.min(remaining, pot)
  return {
    currency,
    limit: state.monthlyLimit,
    spent,
    remaining,
    spendable,
    pot,
    needsTopUp: potNeedsTopUp(state.monthlyLimit, pot, spendable, rolledOver),
    rolledOver,
    set: true
  }
}

/**
 * Reads every monthly allowance the wallet can hold — SOL and each stablecoin it
 * could pay with on this cluster — and the total they come to in the currency
 * the person reads prices in.
 *
 * An allowance is public on-chain state, so no browser connection is needed;
 * only a fresh signup with nothing linked falls back to the connected wallet.
 * The three budgets are separate accounts, which is why the card shows them
 * apart and adds them up only in fiat: lamports and token atoms are not one
 * number until a price turns them into one.
 *
 * Each row reports what is spendable, not just what the month's ceiling leaves:
 * a new month resets the counter without moving any money, so a full ceiling
 * over an empty pot would read as money the person does not have.
 */
export function useProfileAllowance({
  linkedWallet,
  publicKey
}: UseProfileAllowanceOptions) {
  const { t } = useI18n()
  const budgetApi = useBudget()
  const cluster = useRuntimeConfig().public.cluster as string
  const currencies = currenciesForCluster(cluster)
  const { currency: displayCurrency } = useCurrencyPreference()
  const rates = useSolRates()

  const budget = ref<BudgetState | null>(null)
  const budgetPot = ref(0)
  const tokenBudgets = ref<Partial<Record<SupportedCurrency, TokenBudgetState | null>>>({})
  const budgetLoading = ref(false)
  const budgetError = ref('')

  // The API stores the linked wallet as a validated base58 address.
  const allowanceAddress = computed<Address | null>(
    () => (linkedWallet.value as Address | null) ?? publicKey.value
  )

  /** Each currency's figures, in its own units, for the rows the card shows. */
  const rows = computed<ProfileAllowanceRow[]>(() => {
    const now = currentMonthIndex()
    return currencies.map((currency): ProfileAllowanceRow => {
      if (currency === 'sol') {
        return rowFor(currency, budget.value, budgetPot.value, now)
      }
      const state = tokenBudgets.value[currency] ?? null
      return rowFor(currency, state, state?.amount ?? 0, now)
    })
  })

  /** Whether any of the three exists: false is the card's "set one" prompt. */
  const hasBudget = computed(() => rows.value.some(row => row.set))

  /** The allowances a top-up would actually change, in the order the card lists them. */
  const topUpRows = computed(() => rows.value.filter(row => row.needsTopUp))

  /** Whether the profile should offer the top-up nudge at all. */
  const needsTopUp = computed(() => topUpRows.value.length > 0)

  /** The currencies the nudge names, written as symbols ("SOL, USDC"). */
  const topUpCurrencies = computed(() =>
    topUpRows.value.map(row => currencySymbol(row.currency)).join(', ')
  )

  /** True when the month turned over under a pot that was not refilled. */
  const rolledOver = computed(() => topUpRows.value.some(row => row.rolledOver))

  /**
   * What every remaining allowance is worth together, in the display currency.
   * `null` when a rate is missing, so the card shows no total rather than a
   * smaller one that reads as the whole truth.
   */
  const totalRemainingFiat = computed<number | null>(() => {
    let total = 0
    for (const row of rows.value) {
      if (!row.set) continue
      const converted = fiatValueOf(row.currency, row.spendable, rates.value, displayCurrency.value)
      if (converted === null) return null
      total += converted
    }
    return total
  })

  async function refreshBudget() {
    budget.value = null
    budgetPot.value = 0
    tokenBudgets.value = {}
    budgetError.value = ''
    const address = allowanceAddress.value
    if (!address) return

    budgetLoading.value = true
    try {
      const loaded = await budgetApi.loadBudget(address, { force: true })
      budget.value = loaded
      if (loaded) budgetPot.value = await budgetApi.availablePot(loaded)

      for (const currency of currencies) {
        if (currency === 'sol') continue
        const mint = mintForCurrency(currency, cluster)
        if (!mint) continue
        tokenBudgets.value[currency] = await budgetApi.loadTokenBudget(address, mint, {
          force: true
        })
      }
    } catch (error) {
      budgetError.value
        = error instanceof Error ? error.message : t('profile.allowanceError')
    } finally {
      budgetLoading.value = false
    }
  }

  watch(allowanceAddress, refreshBudget, { immediate: true })

  return {
    allowanceAddress,
    rows,
    hasBudget,
    needsTopUp,
    topUpCurrencies,
    rolledOver,
    totalRemainingFiat,
    displayCurrency,
    budgetLoading,
    budgetError
  }
}
