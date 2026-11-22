import type { Address } from '@vue-solana/nuxt/kit'
import type { Ref } from 'vue'
import type { BudgetState, TokenBudgetState } from './budgetMath'
import type { SupportedCurrency } from './usePaymentApi'

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
  /** What is left of the ceiling, in the same units. */
  remaining: number
  /** What the pot or vault holds right now, in the same units. */
  pot: number
  /** False when this wallet has no allowance in this currency at all. */
  set: boolean
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
        const state = budget.value
        const spent = state && state.monthIndex === now ? state.spent : 0
        return {
          currency,
          limit: state?.monthlyLimit ?? 0,
          spent,
          remaining: state ? Math.max(0, state.monthlyLimit - spent) : 0,
          pot: budgetPot.value,
          set: Boolean(state)
        }
      }
      const state = tokenBudgets.value[currency] ?? null
      const spent = state && state.monthIndex === now ? state.spent : 0
      return {
        currency,
        limit: state?.monthlyLimit ?? 0,
        spent,
        remaining: state ? Math.max(0, state.monthlyLimit - spent) : 0,
        pot: state?.amount ?? 0,
        set: Boolean(state)
      }
    })
  })

  /** Whether any of the three exists: false is the card's "set one" prompt. */
  const hasBudget = computed(() => rows.value.some(row => row.set))

  /**
   * What every remaining allowance is worth together, in the display currency.
   * `null` when a rate is missing, so the card shows no total rather than a
   * smaller one that reads as the whole truth.
   */
  const totalRemainingFiat = computed<number | null>(() => {
    let total = 0
    for (const row of rows.value) {
      if (!row.set) continue
      const converted = fiatValueOf(row.currency, row.remaining, rates.value, displayCurrency.value)
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
    totalRemainingFiat,
    displayCurrency,
    budgetLoading,
    budgetError
  }
}
