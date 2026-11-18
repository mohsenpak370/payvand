import { currencySymbol } from './usePaymentApi'
import { budgetAmountLabel, budgetBaseUnits } from '~/utils/budgetAmount'
import { DEFAULT_LIMITS, type BudgetPageState } from './useBudgetPageState'

/**
 * The limit field and the units its amount is written in.
 *
 * Switching currency swaps an untouched default for that currency's own low
 * one, and leaves an edited amount alone: the number means a different sum in
 * each currency, so carrying it across would silently change what is set.
 */
export function useBudgetLimit(state: BudgetPageState) {
  const { activeCurrency, limitAmount } = state

  const unitLabel = computed(() => currencySymbol(activeCurrency.value))

  /** `0.1 SOL`, `5 USDC` — an amount from the allowance in the units on screen. */
  function amountLabel(value: number): string {
    return budgetAmountLabel(activeCurrency.value, value)
  }

  /** What one unit of the selected currency is in base units. */
  function toBaseUnits(value: number): number {
    return budgetBaseUnits(activeCurrency.value, value)
  }

  watch(activeCurrency, (currency, previous) => {
    if (limitAmount.value === DEFAULT_LIMITS[previous]) {
      limitAmount.value = DEFAULT_LIMITS[currency]
    }
  })

  return { unitLabel, amountLabel, toBaseUnits, defaultLimits: DEFAULT_LIMITS }
}
