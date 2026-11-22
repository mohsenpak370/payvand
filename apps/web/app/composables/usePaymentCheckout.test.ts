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
 * fronts the network fee and both record rents, and a wallet that holds them
 * must keep paying them itself; below that the relayer does, and the buyer only
 * signs. The balance is the only thing that decides between the two.
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
  it('pays its own fee and rents when the wallet holds them', async () => {
    const checkout = checkoutFor(quoteFor(false))

    await checkout.pay()

    // Today's path, unchanged: the buyer is its own fee payer, its own sponsor,
    // and the relayer is never asked for anything.
    expect(execute).toHaveBeenCalledTimes(1)
    expect(checkout.signature.value).toBe('SIGNATURE')
    expect(getSponsor).not.toHaveBeenCalled()
    expect(sponsorSettle).not.toHaveBeenCalled()
    expect(signTransactions).not.toHaveBeenCalled()
    expect(buildInstructionMock).toHaveBeenCalledWith(
      'settle_token_payment',
      expect.anything(),
      expect.objectContaining({ sponsor: payer })
    )
  })

  it('asks the relayer to front them when the wallet cannot', async () => {
    walletLamports.value = 0
    getSponsor.mockResolvedValueOnce({
      available: true,
      address: sponsor,
      needs_lamports: 3_944_360,
      per_wallet_daily: 10
    })
    const checkout = checkoutFor(quoteFor(false))

    await checkout.pay()

    // The relayer's key is the fee payer and the instruction's sponsor; the
    // buyer signs its half and nothing is sent from this device.
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

  it('keeps the wallet’s own path when no relayer is available', async () => {
    walletLamports.value = 0

    const checkout = checkoutFor(quoteFor(false))
    await checkout.pay()

    // Asked, refused, and the buyer's own transaction is built anyway: the
    // chain's answer is worth more than a client-side dead end.
    expect(getSponsor).toHaveBeenCalledTimes(1)
    expect(sponsorSettle).not.toHaveBeenCalled()
    expect(execute).toHaveBeenCalledTimes(1)
    expect(checkout.signature.value).toBe('SIGNATURE')
  })
})
