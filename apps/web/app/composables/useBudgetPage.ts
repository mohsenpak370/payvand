import { useBudgetAllowances } from './useBudgetAllowances'
import { useBudgetApply } from './useBudgetApply'
import { useBudgetLimit } from './useBudgetLimit'
import { useBudgetMessages } from './useBudgetMessages'
import { useBudgetPageState } from './useBudgetPageState'
import { useBudgetQuote } from './useBudgetQuote'
import { useBudgetReclaim } from './useBudgetReclaim'

/**
 * The monthly allowance page, wired once.
 *
 * This function is only the wiring: it creates the shared state, composes the
 * modules around it in dependency order, and hands back one flat API. The
 * state owns the refs, the allowances module reads the chain and derives what
 * the card shows, the quote prices the limit on screen, and the apply and
 * reclaim modules are the two directions money can move. The page itself only
 * decides which of those actions to offer.
 */
export function useBudgetPage() {
  const { publicKey, connected } = useSolanaWallet()
  const { client } = useSolanaClient()
  const { execute, loading: txLoading } = useSolanaSignAndSendTransaction()
  const budgetApi = useBudget()

  const state = useBudgetPageState()
  const {
    currencies, activeCurrency, limitAmount, busy, failure, failureTitle, success,
    deviceKeyAddress, recoverableFloat, sweeping, budgetsRead
  } = state

  const {
    activeMint, tokenBudget, view, foreignDevice, liveSessionKeys, refillNeeded,
    refresh, refreshRecoverableFloat
  } = useBudgetAllowances({ state, budgetApi, publicKey })

  const { unitLabel, amountLabel, toBaseUnits, defaultLimits } = useBudgetLimit(state)

  const messages = useBudgetMessages(state)

  const { openCost, openCostMissing } = useBudgetQuote({
    state,
    budgetApi,
    publicKey,
    activeMint,
    view
  })

  const { saveLimit, rotateToThisDevice, refill } = useBudgetApply({
    state,
    messages,
    budgetApi,
    client,
    execute,
    publicKey,
    activeMint,
    tokenBudget,
    view,
    refresh,
    amountLabel,
    toBaseUnits,
    unitLabel
  })

  const { closeBudget, recoverFloat } = useBudgetReclaim({
    state,
    messages,
    budgetApi,
    client,
    execute,
    publicKey,
    activeMint,
    view,
    liveSessionKeys,
    refresh,
    refreshRecoverableFloat,
    unitLabel
  })

  return {
    connected,
    txLoading,
    currencies,
    activeCurrency,
    limitAmount,
    defaultLimits,
    busy,
    failure,
    failureTitle,
    success,
    view,
    foreignDevice,
    deviceKeyAddress,
    recoverableFloat,
    sweeping,
    refillNeeded,
    openCost,
    openCostMissing,
    budgetsRead,
    saveLimit,
    rotateToThisDevice,
    refill,
    closeBudget,
    recoverFloat
  }
}
