import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import {
  buildInstructionMock,
  checkoutFor,
  execute,
  getSponsor,
  mint,
  payer,
  quoteFor,
  reference,
  seller,
  setupCheckout,
  signTransactions,
  sponsor,
  sponsorSettle,
  teardownCheckout,
  walletLamports
} from '../test-utils/usePaymentCheckout'

/**
 * `pay()` is the last thing between an invoice and a signature, and one of its
 * preflight checks exists for a state no button can reach: the treasury has no
 * account for the mint, so the program's fee has nowhere to go and the settle is
 * rejected — after the buyer signed. The page holds the button for that, so the
 * test calls `pay()` directly, the way the stage machine would. If the guard
 * goes away, `pay()` builds and sends the transaction and this fails.
 *
 * The other subject here is who pays the settle's lamports. A token settle
 * fronts the network fee and both record rents, and Payvand's own fee is what
 * pays for them: every token payment asks the relayer first, so a payer holding
 * no SOL can pay. Only a payment no relayer will take falls back to the wallet
 * paying them itself — and when it cannot, the refusal comes before a signature.
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

describe('who pays the settle', () => {
  it('lets the relayer pay even when the wallet could', async () => {
    getSponsor.mockResolvedValueOnce({
      available: true,
      address: sponsor,
      needs_lamports: 3_944_360,
      per_wallet_daily: 10
    })
    const checkout = checkoutFor(quoteFor(false))

    await checkout.pay()

    // The relayer's key is the fee payer and the instruction's sponsor; the
    // buyer signs its half and nothing is sent from this device. The wallet's
    // own balance is not consulted: paying from Payvand's fee is the point.
    expect(execute).not.toHaveBeenCalled()
    expect(signTransactions).toHaveBeenCalledTimes(1)
    expect(buildInstructionMock).toHaveBeenCalledWith(
      'settle_token_payment',
      expect.anything(),
      expect.objectContaining({ sponsor })
    )
    expect(sponsorSettle).toHaveBeenCalledTimes(1)
    expect(sponsorSettle.mock.calls[0]![0]).toMatchObject({
      transaction: 'BASE64_WIRE',
      buyer: payer,
      seller,
      reference,
      mint,
      amount: 1_500_000
    })
    expect(checkout.signature.value).toBe('SPONSORED_SIGNATURE')
    expect(checkout.failure.value).toBe('')
  })

  it('pays its own fee and rents when no relayer takes the payment', async () => {
    const checkout = checkoutFor(quoteFor(false))

    await checkout.pay()

    // Asked first, refused, and the funded wallet pays exactly as it always
    // did: the chain's answer is worth more than a client-side dead end.
    expect(getSponsor).toHaveBeenCalledTimes(1)
    expect(execute).toHaveBeenCalledTimes(1)
    expect(checkout.signature.value).toBe('SIGNATURE')
    expect(sponsorSettle).not.toHaveBeenCalled()
    expect(signTransactions).not.toHaveBeenCalled()
    expect(buildInstructionMock).toHaveBeenCalledWith(
      'settle_token_payment',
      expect.anything(),
      expect.objectContaining({ sponsor: payer })
    )
  })

  it('refuses before a signature when there is no relayer and no SOL', async () => {
    walletLamports.value = 0

    const checkout = checkoutFor(quoteFor(false))
    await checkout.pay()

    // Nothing can front the fee, so the payment stops here and says why
    // instead of failing inside a wallet prompt the buyer already approved.
    expect(checkout.failure.value).toBe('errors.paymentNeedsSol')
    expect(checkout.signature.value).toBeNull()
    expect(execute).not.toHaveBeenCalled()
    expect(signTransactions).not.toHaveBeenCalled()
    expect(buildInstructionMock).not.toHaveBeenCalled()
  })
})
