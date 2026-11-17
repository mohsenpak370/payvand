import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import {
  buildInstructionMock,
  checkoutFor,
  execute,
  quoteFor,
  setupCheckout,
  teardownCheckout
} from '../test-utils/usePaymentCheckout'

/**
 * `pay()` is the last thing between an invoice and a signature, and one of its
 * preflight checks exists for a state no button can reach: the treasury has no
 * account for the mint, so the program's fee has nowhere to go and the settle is
 * rejected — after the buyer signed. The page holds the button for that, so the
 * test calls `pay()` directly, the way the stage machine would. If the guard
 * goes away, `pay()` builds and sends the transaction and this fails.
 */

beforeEach(setupCheckout)
afterEach(teardownCheckout)

describe('pay()', () => {
  it('refuses before building anything when the treasury has no account for the mint', async () => {
    const checkout = checkoutFor(quoteFor(true))

    await checkout.pay()

    expect(checkout.failure.value).toBe('errors.treasuryTokenAccountMissing')
    expect(buildInstructionMock).not.toHaveBeenCalled()
    expect(execute).not.toHaveBeenCalled()
    expect(checkout.signature.value).toBeNull()
  })

  it('builds and sends the same invoice once that account is there', async () => {
    const checkout = checkoutFor(quoteFor(false))

    await checkout.pay()

    expect(checkout.failure.value).toBe('')
    expect(buildInstructionMock).toHaveBeenCalledWith(
      'settle_token_payment',
      expect.anything(),
      expect.anything()
    )
    expect(execute).toHaveBeenCalledTimes(1)
    expect(checkout.signature.value).toBe('SIGNATURE')
  })
})
