import type { Address } from '@vue-solana/nuxt/kit'
import {
  remainingThisMonth as remainingThisMonthFn,
  tokenRemainingThisMonth as tokenRemainingThisMonthFn,
  type BudgetState,
  type TokenBudgetState
} from './budgetMath'
import {
  loadBudgetAccount,
  loadTokenBudgetAccount,
  tokenVault
} from './budgetAccounts'
import {
  preflightBudgetTransaction,
  sweepSessionFloat as sweepSessionFloatTx,
  sweepableSessionFloat
} from './budgetTransactions'
import { createSolBudgetOps } from './budgetSolOps'
import { createTokenBudgetOps } from './budgetTokenOps'

export {
  BUDGET_SPACE,
  TOKEN_BUDGET_SPACE,
  currentMonthIndex,
  deriveBudgetPda,
  deriveTokenBudgetPda,
  remainingThisMonth,
  sweepableLamports,
  tokenFloatNeed,
  tokenFloatTopUp,
  tokenOpenCost,
  tokenRemainingThisMonth,
  transferLamportsInstruction
} from './budgetMath'
export type { BudgetState, TokenBudgetState, TokenOpenCostInput } from './budgetMath'
export { clearBudgetAccountCache } from './budgetAccounts'

/** Reads and interprets a buyer's budget straight from the cluster. */
export function useBudget() {
  const { client } = useSolanaClient()
  // `open_budget` tops the session key up to `Config::session_fee_float`, so
  // the quote reads the same account the program will.
  const feeConfig = useFeeConfig()

  const rentCache = new Map<number, number>()
  async function rentMinimum(space: number): Promise<number> {
    const hit = rentCache.get(space)
    if (hit !== undefined) return hit
    const lamports = Number(
      await client.rpc.getMinimumBalanceForRentExemption(BigInt(space)).send()
    )
    rentCache.set(space, lamports)
    return lamports
  }

  const loadBudget = (buyer: Address, options: { force?: boolean } = {}) =>
    loadBudgetAccount(client, buyer, options)
  const loadTokenBudget = (buyer: Address, mint: Address, options: { force?: boolean } = {}) =>
    loadTokenBudgetAccount(client, buyer, mint, options)

  const sol = createSolBudgetOps({ client, feeConfig, rentMinimum, loadBudget })
  const token = createTokenBudgetOps({ client, rentMinimum, loadTokenBudget })

  return {
    loadBudget,
    remainingThisMonth: (budget: BudgetState) => remainingThisMonthFn(budget),
    ...sol,
    preflight: (wire: Parameters<typeof preflightBudgetTransaction>[1]) =>
      preflightBudgetTransaction(client, wire),
    sweepableFloat: (sessionAddress: Address) => sweepableSessionFloat(client, sessionAddress),
    sweepSessionFloat: (buyer: Address) => sweepSessionFloatTx(client, buyer),
    tokenVault,
    loadTokenBudget,
    tokenRemainingThisMonth: (budget: TokenBudgetState) => tokenRemainingThisMonthFn(budget),
    ...token
  }
}
