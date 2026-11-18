import type { Address } from '@vue-solana/nuxt/kit'
import type { Ref } from 'vue'
import type { BudgetState, TokenBudgetState } from './budgetMath'
import type { SupportedCurrency } from './usePaymentApi'

/**
 * The allowance card's mutable state, created once and shared by every module
 * on the page.
 *
 * The selected currency lives here rather than in the quote module because the
 * reads, the quote and the actions all follow the same choice: the card, the
 * field and every message name the allowance that switch points at.
 */
export interface BudgetPageState {
  /** Cluster the stablecoin mints are picked for. */
  cluster: string
  currencies: SupportedCurrency[]
  activeCurrency: Ref<SupportedCurrency>
  limitAmount: Ref<string>
  busy: Ref<boolean>
  failure: Ref<string>
  failureTitle: Ref<string>
  success: Ref<string>
  budget: Ref<BudgetState | null>
  /** The SOL pot above its rent-exempt reserve: what invoices are paid from. */
  budgetPot: Ref<number>
  tokenBudgets: Ref<Partial<Record<SupportedCurrency, TokenBudgetState | null>>>
  /** The session key this device holds, if any — the only key that can pay here. */
  deviceKeyAddress: Ref<Address | null>
  /** What this device's key still holds that no allowance can spend. */
  recoverableFloat: Ref<number>
  sweeping: Ref<boolean>
  /**
   * Whether the last read of the budgets succeeded. A failed read leaves them
   * null, which must not be mistaken for "no allowance needs this key": that
   * guess would sweep a live key's float and forget it.
   */
  budgetsRead: Ref<boolean>
}

/**
 * Low on purpose: the session key pays without the wallet, so the limit is
 * also the most a device holding it can ever spend on its own.
 */
export const DEFAULT_LIMITS: Record<SupportedCurrency, string> = {
  sol: '0.1',
  usdc: '1',
  eurc: '1'
}

export function useBudgetPageState(): BudgetPageState {
  const { t } = useI18n()
  const cluster = useRuntimeConfig().public.cluster as string

  // One allowance per currency: SOL holds lamports, each stablecoin holds its
  // own tokens. The card follows whichever currency is selected.
  return {
    cluster,
    currencies: currenciesForCluster(cluster),
    activeCurrency: ref<SupportedCurrency>('sol'),
    limitAmount: ref(DEFAULT_LIMITS.sol),
    busy: ref(false),
    failure: ref(''),
    failureTitle: ref(t('budget.errors.genericTitle')),
    success: ref(''),
    budget: ref<BudgetState | null>(null),
    budgetPot: ref(0),
    tokenBudgets: ref<Partial<Record<SupportedCurrency, TokenBudgetState | null>>>({}),
    deviceKeyAddress: ref<Address | null>(null),
    recoverableFloat: ref(0),
    sweeping: ref(false),
    budgetsRead: ref(false)
  }
}
