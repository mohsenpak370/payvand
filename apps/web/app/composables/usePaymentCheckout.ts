import { useCheckoutBudget } from './useCheckoutBudget'
import { useCheckoutChain } from './useCheckoutChain'
import { useCheckoutCosts } from './useCheckoutCosts'
import { useCheckoutDiscount } from './useCheckoutDiscount'
import { useCheckoutEligibility } from './useCheckoutEligibility'
import { useCheckoutInvoice } from './useCheckoutInvoice'
import { useCheckoutLoader } from './useCheckoutLoader'
import { useCheckoutPayment } from './useCheckoutPayment'
import { useCheckoutState } from './useCheckoutState'

/**
 * Resolves an invoice from the shared link and carries it to payment.
 *
 * The three ways to pay are deliberately separate states rather than one button
 * with a fallback chain, because each one needs the buyer to do something
 * different — nothing, approve, or open a different app.
 *
 * This function is now only the wiring: it creates the shared state, composes
 * the modules around it in dependency order, and hands back one flat API. The
 * state owns the mutable refs, the invoice module derives what the buyer sees,
 * eligibility decides what they can do, and the loader sequences reads. No
 * module reaches into another's private state.
 */
export function usePaymentCheckout() {
  const api = usePaymentApi()
  const { client } = useSolanaClient()
  const wallet = useSolanaWallet()
  const { publicKey } = wallet
  const { execute } = useSolanaSignAndSendTransaction()
  const { execute: signTransactions } = useSolanaSignTransactions()
  const budget = useBudget()

  const state = useCheckoutState()
  const {
    payload, code, record, loading, error, failure, busy, signature,
    selectedCurrency, activePda, discount, discountInput, discountBusy, discountError
  } = state

  const {
    options, selected, offersChoice, priceAmount, payableAmount, payableLamports,
    priceLamports, paymentName, paymentDescription, paymentMemo, items, addressFor
  } = useCheckoutInvoice(state)

  const { costs, costsError, refreshCosts, treasuryTokenAccountMissing } = useCheckoutCosts(
    payload,
    selected,
    payableAmount
  )

  const {
    budgetState,
    tokenBudgetState,
    budgetObstacle,
    sessionReady,
    refreshBudget,
    activeBudget,
    canPaySilently
  } = useCheckoutBudget({
    payer: () => publicKey.value,
    payload,
    selected,
    payableAmount,
    budget
  })

  const {
    settled,
    canPayFromAllowance,
    canPayWithWallet,
    blockedReason,
    stage
  } = useCheckoutEligibility({
    state,
    wallet,
    treasuryTokenAccountMissing,
    canPaySilently,
    activeBudget,
    sessionReady
  })

  const {
    stopPolling,
    startPolling,
    watchAddresses,
    settleFromChain,
    restartChainReads
  } = useCheckoutChain({ payload, record, settled, activePda, options, api, client })

  const { holding, holdProgress, beginHold, stopHold, pay } = useCheckoutPayment({
    api,
    budget,
    client,
    execute,
    signTransactions,
    payer: () => publicKey.value,
    payload,
    record,
    busy,
    signature,
    failure,
    settled,
    canPaySilently,
    canPayFromAllowance,
    selected,
    payableAmount,
    costs,
    costsError,
    refreshCosts,
    addressFor
  })

  const { applyDiscount, clearDiscount } = useCheckoutDiscount({
    state,
    api,
    priceAmount,
    payableAmount,
    selected,
    buyer: () => publicKey.value?.toString() ?? null,
    stopHold,
    refreshCosts,
    refreshBudget,
    addressFor
  })

  const { load, selectCurrency } = useCheckoutLoader({
    state,
    api,
    wallet,
    selected,
    payableAmount,
    addressFor,
    stopHold,
    applyDiscount,
    refreshCosts,
    refreshBudget,
    stopPolling,
    startPolling,
    watchAddresses,
    settleFromChain,
    restartChainReads
  })

  return {
    payload,
    code,
    options,
    selected,
    selectedCurrency,
    selectCurrency,
    offersChoice,
    payableAmount,
    priceAmount,
    activeBudget,
    tokenBudgetState,
    record,
    loading,
    error,
    failure,
    busy,
    signature,
    discount,
    discountInput,
    discountBusy,
    discountError,
    budgetState,
    budgetObstacle,
    sessionReady,
    holding,
    holdProgress,
    beginHold,
    stopHold,
    payableLamports,
    priceLamports,
    costs,
    costsError,
    settled,
    paymentName,
    paymentDescription,
    paymentMemo,
    items,
    canPaySilently,
    canPayFromAllowance,
    canPayWithWallet,
    blockedReason,
    treasuryTokenAccountMissing,
    stage,
    load,
    pay,
    applyDiscount,
    clearDiscount
  }
}
