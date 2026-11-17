import { address } from '@vue-solana/nuxt/kit'
import { paymentOptions, type PaymentQrPayload, type PaymentOption } from './usePaymentApi'
import { derivePaymentPda, deriveTokenPaymentPda } from './usePaymentProgram'

/**
 * Paying from the allowance needs no wallet approval, so the buyer's consent is
 * the press itself: the button has to be held for this long, or nothing is
 * sent. Nothing pays on its own, so a reload, a back navigation or a link
 * preview can never charge anyone.
 */
export const HOLD_TO_PAY_MS = 3_000
/** How often the hold's fill and elapsed time are recomputed. */
export const HOLD_TICK_MS = 50
export const STATUS_POLL_MS = 5_000

/**
 * What the page can offer right now. The order is the priority: a paid invoice
 * is a receipt, and the allowance is always preferred over a wallet approval —
 * but only the buyer's press ever starts a payment.
 *
 * `pick`   no wallet in this browser yet → offer the wallet list
 * `wallet` a connected wallet approves it (`budgetState === null` there means
 *          no allowance exists yet, so the page offers to set one up)
 * `setup`  a budget exists, but this device holds no session key for it
 * `silent` the allowance covers it: one deliberate, held press pays it
 */
export type CheckoutStage
  = | 'loading'
    | 'invalid'
    | 'settled'
    | 'confirming'
    | 'silent'
    | 'wallet'
    | 'setup'
    | 'pick'

/**
 * Proves a code names what it charges: the program binds the seller, the
 * reference, the currency and the price into each invoice address, so details
 * edited apart cannot derive the address the code carries. Returns the options
 * it checked — every way that code can be paid.
 *
 * Shared with the scanner, which checks a code before showing any payment UI:
 * a code read off a printout has to fail at the camera, not one screen later.
 */
export async function verifyPaymentCode(parsed: PaymentQrPayload): Promise<PaymentOption[]> {
  const offered = paymentOptions(parsed)
  if (!offered.length) throw new Error(appT('errors.noPrice'))

  for (const option of offered) {
    const derived = option.currency === 'sol'
      ? await derivePaymentPda(
          address(parsed.seller),
          address(parsed.reference),
          option.amount
        )
      : await deriveTokenPaymentPda(
          address(parsed.seller),
          address(parsed.reference),
          address(option.mint),
          option.amount
        )
    if (derived !== option.payment_pda) {
      throw new Error(appT('errors.detailsMismatch'))
    }
  }

  return offered
}
