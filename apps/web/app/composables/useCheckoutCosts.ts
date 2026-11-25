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
      // One signature: this figure is what the payment costs the wallet when it
      // pays its own way, which is the only case the checkout shows it. A
      // settlement a relayer fee-pays carries two, and there the fee line reads
      // that the fee is included in the price instead — `estimateInvoiceCosts`
      // takes `signatures: 2` from a caller that knows a relayer will submit.
      const quote = await estimateInvoiceCosts(payableAmount.value, option.currency, {
        mint: option.mint || undefined,
        seller: payload.value?.seller ? address(payload.value.seller) : undefined,
        signatures: 1,
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
