import { address } from '@vue-solana/nuxt/kit'
import type { ComputedRef, Ref } from 'vue'
import type { InvoiceCosts } from '~/utils/invoiceCost'
import type { PaymentOption, PaymentQrPayload } from './usePaymentApi'

/** The fee and rent this invoice costs, quoted from the on-chain policy. */
export function useCheckoutCosts(
  payload: Ref<PaymentQrPayload | null>,
  selected: ComputedRef<PaymentOption>,
  payableAmount: ComputedRef<number>
) {
  const { estimateInvoiceCosts } = useInvoiceCosts()

  // `null` until the first read, and again if `Config` cannot be read: the
  // checkout shows no total rather than one that assumes a zero fee.
  const costs = ref<InvoiceCosts | null>(null)
  const costsError = ref('')

  async function refreshCosts(): Promise<InvoiceCosts | null> {
    const option = selected.value
    try {
      // Two signatures: the relayer pays for every wallet payment and the
      // program charges the seller the network fee of both (the relayer's and
      // the buyer's), so the seller's proceeds and the smallest payable amount
      // are the relayer path's. A wallet that has to pay its own way carries one
      // and is charged a signature less — this quote is the larger of the two,
      // so the guard on the price never lets through an amount the chain
      // refuses. See `docs/payment-costs.md`.
      const quote = await estimateInvoiceCosts(payableAmount.value, option.currency, {
        mint: option.mint || undefined,
        seller: payload.value?.seller ? address(payload.value.seller) : undefined,
        signatures: 2,
        // The code's cluster, not this page's: pricing an invoice on another
        // cluster from the policy here would quote a fee the program will not
        // charge.
        cluster: payload.value?.cluster
      })
      costs.value = quote
      costsError.value = ''
      return quote
    } catch (cause) {
      costs.value = null
      costsError.value = cause instanceof Error ? cause.message : appT('errors.feeConfigUnavailable')
      return null
    }
  }

  /**
   * The fee's destination for this mint does not exist yet, so the program
   * would reject the settle after the buyer signed. That account is the
   * protocol's own setup — one call of `open_treasury_token_account` by the
   * deployer — never something a buyer can pay or fix, so the page says what is
   * missing and holds the button.
   */
  const treasuryTokenAccountMissing = computed(
    () => costs.value?.treasuryTokenAccountMissing === true
  )

  return { costs, costsError, refreshCosts, treasuryTokenAccountMissing }
}
