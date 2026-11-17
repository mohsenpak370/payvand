import type { ComputedRef, Ref } from 'vue'
import type { BudgetState, TokenBudgetState } from './useBudget'
import type { CheckoutStage } from './checkoutVerification'
import type { PaymentCheckoutState } from './useCheckoutState'

/**
 * The derived answers the page renders from: is this invoice still payable,
 * which path can pay it, and which screen that puts the buyer on. Every rule
 * lives here; the orchestrator only supplies the facts it reads.
 */
export function useCheckoutEligibility(deps: {
  state: PaymentCheckoutState
  wallet: ReturnType<typeof useSolanaWallet>
  treasuryTokenAccountMissing: ComputedRef<boolean>
  canPaySilently: ComputedRef<boolean>
  activeBudget: ComputedRef<BudgetState | TokenBudgetState | null>
  sessionReady: Ref<boolean>
}) {
  const { state, wallet, treasuryTokenAccountMissing, canPaySilently, activeBudget, sessionReady } = deps
  const { publicKey, connected, canSignTransaction } = wallet
  const { payload, record, loading, error, busy, signature } = state

  const settled = computed(() => record.value?.status === 'settled')

  /**
   * The invoice itself, whatever signs: nothing is sent while one payment is in
   * flight, after one landed, or to a seller whose fee account does not exist.
   * An unpaid invoice is a record that is not settled — `pending` and the older
   * `missing` both mean nobody has paid it yet.
   *
   * `signature` is this page's own send: the chain has it, but the record the
   * page reads comes from the API and can still say `pending`. Pressing again in
   * that window only earns `AlreadySettled` from the program, so the button
   * stays away until the record catches up. A payment the chain already held
   * when the page loaded is in the record too — `load()` asks the chain.
   */
  const invoicePayable = computed(
    () =>
      Boolean(record.value)
      && !settled.value
      && !busy.value
      && !signature.value
      && !treasuryTokenAccountMissing.value
  )

  /** Both payment paths send as a connected wallet, whatever signs the payment. */
  const walletReady = computed(() => connected.value && Boolean(publicKey.value))

  /**
   * The session key signs an allowance payment, so a wallet that cannot sign is
   * no obstacle to it — the wallet only says who is paying. Gating this path on
   * the wallet's own signer is what made the hold button unavailable on wallets that
   * report they cannot sign.
   */
  const canPayFromAllowance = computed(
    () => walletReady.value && invoicePayable.value && canPaySilently.value
  )

  /** A wallet payment is the wallet's own approval, so that wallet must sign. */
  const canPayWithWallet = computed(
    () => walletReady.value && canSignTransaction.value && invoicePayable.value
  )

  /**
   * What the wallet button cannot do, in the buyer's words. It is only read in
   * the two stages that render that button, and both are reached only by a
   * connected wallet on an unpaid invoice — a disconnected page shows the wallet
   * list, and a paid one shows the receipt — so the only thing left to explain is
   * a wallet that will not sign, which the allowance path does not need.
   */
  const blockedReason = computed(() =>
    canSignTransaction.value ? '' : appT('checkout.walletCannotSign')
  )

  const stage = computed<CheckoutStage>(() => {
    if (loading.value) return 'loading'
    if (error.value || !payload.value) return 'invalid'
    if (settled.value) return 'settled'
    // Sent from here, not yet settled: the button stays out of reach while the
    // record catches up, and the poll below is what brings it in.
    if (signature.value) return 'confirming'
    if (!connected.value || !publicKey.value) return 'pick'
    if (canPaySilently.value) return 'silent'
    // A budget this device cannot sign for is a one-time setup step, not a
    // reason to fall back to a popup on every payment.
    if (activeBudget.value && !sessionReady.value) return 'setup'
    return 'wallet'
  })

  return {
    settled,
    invoicePayable,
    walletReady,
    canPayFromAllowance,
    canPayWithWallet,
    blockedReason,
    stage
  }
}
