import type { Address } from '@vue-solana/nuxt/kit'
import type { ComputedRef } from 'vue'
import type { InvoiceCosts } from '~/utils/invoiceCost'
import type { PaymentOption } from './usePaymentApi'
import type { PaymentCheckoutState } from './useCheckoutState'

/**
 * A discount typed by the buyer, or carried by the invoice's own code. The
 * discount refs live in the shared state so the amount can read them while they
 * are still being written here — pricing and discount are two views of one price.
 */
export function useCheckoutDiscount(deps: {
  state: PaymentCheckoutState
  api: ReturnType<typeof usePaymentApi>
  priceAmount: ComputedRef<number>
  payableAmount: ComputedRef<number>
  selected: ComputedRef<PaymentOption>
  stopHold: () => void
  refreshCosts: () => Promise<InvoiceCosts | null>
  refreshBudget: () => Promise<void>
  addressFor: (option: PaymentOption, amount: number) => Promise<Address>
}) {
  const { state, api, priceAmount, payableAmount, selected } = deps
  const { stopHold, refreshCosts, refreshBudget, addressFor } = deps
  const { payload, activePda, discount, discountInput, discountBusy, discountError } = state

  /** Ask the API to price the invoice with a code — typed, or carried by it. */
  async function applyDiscount(next = discountInput.value) {
    const current = payload.value
    if (!current?.seller_user) return

    // The price is about to move, so a held press must stop and let the buyer
    // see what they are now paying.
    stopHold()
    discountBusy.value = true
    discountError.value = ''
    try {
      const preview = await api.validateDiscount({
        code: next,
        seller: current.seller,
        // The API owns the percentage; the price it applies to is the one the
        // buyer is looking at, in whichever currency that is.
        price_lamports: priceAmount.value
      })
      discount.value = preview
      // The buyer is now looking at a different price, so the fee and rent
      // split moves with it.
      void refreshCosts()
      discountInput.value = ''
      // The discounted amount derives its own address — watch and pay that.
      activePda.value = (await addressFor(selected.value, payableAmount.value)).toString()
      await refreshBudget()
    } catch (cause) {
      discountError.value = apiErrorMessage(cause, appT('errors.discountFailed'))
    } finally {
      discountBusy.value = false
    }
  }

  function clearDiscount() {
    discount.value = null
    discountInput.value = ''
    discountError.value = ''
    activePda.value = selected.value.payment_pda || null
    void Promise.all([refreshCosts(), refreshBudget()])
  }

  return { discount, discountInput, discountBusy, discountError, applyDiscount, clearDiscount }
}
