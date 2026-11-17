import type { Address } from '@vue-solana/nuxt/kit'
import type { ComputedRef } from 'vue'
import type { InvoiceCosts } from '~/utils/invoiceCost'
import type { PaymentOption, SupportedCurrency } from './usePaymentApi'
import { verifyPaymentCode } from './checkoutVerification'
import type { PaymentCheckoutState } from './useCheckoutState'

/**
 * Turns a scanned code into a loaded invoice, and keeps the two live readings in
 * step with it: a currency switch moves the watched address, and a wallet that
 * arrives late re-reads the allowance. Sequencing lives here; the modules it
 * drives own the state they read. Nothing here starts a payment on its own.
 */
export function useCheckoutLoader(deps: {
  state: PaymentCheckoutState
  api: ReturnType<typeof usePaymentApi>
  wallet: ReturnType<typeof useSolanaWallet>
  selected: ComputedRef<PaymentOption>
  payableAmount: ComputedRef<number>
  addressFor: (option: PaymentOption, amount: number) => Promise<Address>
  stopHold: () => void
  applyDiscount: (code?: string) => Promise<void>
  refreshCosts: () => Promise<InvoiceCosts | null>
  refreshBudget: () => Promise<void>
  stopPolling: () => void
  startPolling: () => void
  watchAddresses: () => (string | null)[]
  settleFromChain: (pdas: (string | null)[]) => Promise<void>
  restartChainReads: () => void
}) {
  const { state, api, wallet, selected, payableAmount, addressFor, stopHold, applyDiscount } = deps
  const { refreshCosts, refreshBudget, stopPolling, startPolling } = deps
  const { watchAddresses, settleFromChain, restartChainReads } = deps
  const { payload, code, record, loading, error, failure, signature, activePda } = state
  const { discount, discountInput, discountError, selectedCurrency } = state

  /**
   * Switching currency moves the invoice to a different on-chain address, so
   * the watch address and the allowance follow it, and a held press is dropped.
   */
  async function selectCurrency(currency: SupportedCurrency) {
    if (currency === selectedCurrency.value) return
    stopHold()
    selectedCurrency.value = currency
    const option = selected.value
    if (option) {
      activePda.value = (await addressFor(option, payableAmount.value)).toString()
    }
    await Promise.all([refreshCosts(), refreshBudget()])
    // The currency being switched to may be the one already paid.
    await settleFromChain([activePda.value])
    // A different address is a new question: its reads start over at the first
    // gap, so the currency just chosen is checked on the next tick.
    restartChainReads()
  }

  async function load(source: string) {
    loading.value = true
    error.value = ''
    failure.value = ''
    signature.value = null
    record.value = null
    payload.value = null
    activePda.value = null
    discount.value = null
    discountInput.value = ''
    discountError.value = ''
    stopHold()
    stopPolling()

    try {
      const parsed = parseQrPayload(source)

      // The QR must re-encode the invoice, not the link it arrived on.
      code.value = unwrapQrText(source)
      payload.value = parsed

      const offered = await verifyPaymentCode(parsed)

      // SOL leads when the seller takes it, so the first price a buyer sees is
      // the one most of them can pay.
      selectedCurrency.value = offered.some(option => option.currency === 'sol')
        ? 'sol'
        : offered[0]!.currency
      activePda.value = selected.value.payment_pda
      record.value = await api.getPayment(activePda.value, {
        cluster: parsed.cluster,
        endpoint: parsed.endpoint
      })

      // A code the invoice carries applies itself, so the first price the
      // buyer sees is the one they pay. `applyDiscount` already quotes costs
      // and budget for that price, so a load that applied it must not run the
      // same pair again. A carried code that failed validation left `discount`
      // empty and still needs the full-price quote.
      if (parsed.discount) await applyDiscount(parsed.discount.code)
      if (!discount.value) await Promise.all([refreshCosts(), refreshBudget()])

      // The record read above is what the API's node answers. It is `confirmed`
      // now, so it trails a payment by a slot rather than by finalization — but a
      // reload can still land inside that slot. Every address this code can be
      // paid on is checked, so a payment in another currency counts too.
      await settleFromChain(watchAddresses())

      restartChainReads()
      startPolling()
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : appT('errors.unreadableLink')
    } finally {
      loading.value = false
    }
  }

  /**
   * A wallet that arrives late — the extension finishing its injection, or the
   * buyer returning from a wallet app — decides which button the page can
   * offer, so the allowance is read again. It never sends anything by itself.
   */
  watch(wallet.publicKey, async () => {
    await refreshBudget()
  })

  onBeforeUnmount(() => {
    stopHold()
    stopPolling()
  })

  return { load, selectCurrency }
}
