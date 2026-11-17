import { address, type Address } from '@vue-solana/nuxt/kit'
import type { PaymentItem, PaymentOption } from './usePaymentApi'
import { discountedLamports, paymentOptions } from './usePaymentApi'
import { derivePaymentPda, deriveTokenPaymentPda } from './usePaymentProgram'
import type { PaymentCheckoutState } from './useCheckoutState'

/**
 * The invoice as the buyer sees it: which currency is selected, what it costs
 * after any discount, and the details that ride along in the code. The discount
 * ref comes from the shared state, so the amount needs no knowledge of the
 * module that writes it.
 */
export function useCheckoutInvoice(state: PaymentCheckoutState) {
  const { payload, record, discount, selectedCurrency } = state

  // The seller decides which currencies an invoice takes; the buyer only
  // picks among them. `selected` is the one being priced and paid.
  const options = computed<PaymentOption[]>(() =>
    payload.value ? paymentOptions(payload.value) : []
  )
  const selected = computed<PaymentOption>(() => {
    return (
      options.value.find(option => option.currency === selectedCurrency.value)
      ?? options.value[0]
      ?? { currency: 'sol', mint: '', decimals: 9, amount: 0, payment_pda: '' }
    )
  })
  const offersChoice = computed(() => options.value.length > 1)

  /** What the chosen currency is charged, after any discount, in base units. */
  const payableAmount = computed(() =>
    discount.value
      ? discountedLamports(selected.value.amount, discount.value.percent_off)
      : selected.value.amount
  )
  const priceAmount = computed(() => selected.value.amount)
  // Kept for the SOL wording elsewhere on the page: an invoice priced in SOL.
  const payableLamports = computed(() => (selected.value.currency === 'sol' ? payableAmount.value : 0))
  const priceLamports = computed(() => (selected.value.currency === 'sol' ? priceAmount.value : 0))

  // Name and description ride in the code itself: the chain stores only the
  // memo, so a record read back by id comes without them.
  const paymentName = computed(() => record.value?.name || payload.value?.name || '')
  const paymentDescription = computed(
    () => record.value?.description || payload.value?.description || ''
  )
  const paymentMemo = computed(() => record.value?.memo || payload.value?.memo || '')
  // The receipt, if the seller built this invoice from their shop. It lives in
  // the code, not the account, so a chain read never loses it.
  const items = computed<PaymentItem[]>(() => payload.value?.items ?? [])

  /** The address the chosen currency, at the price shown, is watched on. */
  async function addressFor(option: PaymentOption, amount: number): Promise<Address> {
    const seller = address(payload.value?.seller ?? '')
    const reference = address(payload.value?.reference ?? '')
    if (option.currency === 'sol') {
      return await derivePaymentPda(seller, reference, amount)
    }
    return await deriveTokenPaymentPda(seller, reference, address(option.mint), amount)
  }

  return {
    options,
    selected,
    offersChoice,
    priceAmount,
    payableAmount,
    payableLamports,
    priceLamports,
    paymentName,
    paymentDescription,
    paymentMemo,
    items,
    addressFor
  }
}
