import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import {
  buildInstructionMock,
  checkoutFor,
  checkoutForSol,
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
  walletLamports,
  walletTokenAccount,
  walletTokens
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

/**
 * A sponsored settle is compiled with the relayer as fee payer, and the wallet
 * has nothing to simulate while that signature is empty, so a wallet that is
 * asked to sign one shows an estimate that has nothing to do with the price.
 * These are the two answers that keep that from happening on a payment the
 * wallet can make itself — and the refusal that stops a payment it cannot make
 * at all, which the chain would reject after the buyer had signed.
 */
describe('a SOL price the wallet has to pay', () => {
  const price = 1_000_000_000
  const relayer = {
    available: true,
    address: sponsor,
    needs_lamports: 3_944_360,
    per_wallet_daily: 10
  }

  it('pays its own fee and rents when it can, so the preview is the price', async () => {
    getSponsor.mockResolvedValueOnce(relayer)
    walletLamports.value = price + 3_000_000

    const checkout = checkoutForSol(price)
    await checkout.pay()

    // One signer, so the wallet simulates a payment it can actually price.
    expect(execute).toHaveBeenCalledTimes(1)
    expect(signTransactions).not.toHaveBeenCalled()
    expect(sponsorSettle).not.toHaveBeenCalled()
    expect(buildInstructionMock).toHaveBeenCalledWith(
      'settle_payment',
      expect.objectContaining({ amount: price }),
      expect.objectContaining({ sponsor: payer })
    )
    expect(checkout.signature.value).toBe('SIGNATURE')
  })

  it('hands it to the relayer when it holds the price but not the rent', async () => {
    getSponsor.mockResolvedValueOnce(relayer)
    walletLamports.value = price

    const checkout = checkoutForSol(price)
    await checkout.pay()

    expect(execute).not.toHaveBeenCalled()
    expect(sponsorSettle).toHaveBeenCalledTimes(1)
    expect(sponsorSettle.mock.calls[0]![0]).toMatchObject({
      transaction: 'BASE64_WIRE',
      buyer: payer,
      seller,
      reference,
      mint: '',
      amount: price
    })
    expect(buildInstructionMock).toHaveBeenCalledWith(
      'settle_payment',
      expect.anything(),
      expect.objectContaining({ sponsor })
    )
    expect(checkout.signature.value).toBe('SPONSORED_SIGNATURE')
  })

  it('refuses a price the wallet does not hold, before the wallet opens', async () => {
    getSponsor.mockResolvedValueOnce(relayer)
    walletLamports.value = 5_500_000

    const checkout = checkoutForSol(price)
    await checkout.pay()

    // A relayer covers the fee and the rents, never the price, so nothing can
    // be built: the buyer is told what the invoice costs instead.
    expect(checkout.failure.value).toBe('errors.walletShortOnPrice')
    expect(buildInstructionMock).not.toHaveBeenCalled()
    expect(execute).not.toHaveBeenCalled()
    expect(signTransactions).not.toHaveBeenCalled()
    expect(sponsorSettle).not.toHaveBeenCalled()
  })
})

describe('a send the API refuses', () => {
  it('shows the API reason instead of the bare status', async () => {
    getSponsor.mockResolvedValueOnce({
      available: true,
      address: sponsor,
      needs_lamports: 3_944_360,
      per_wallet_daily: 10
    })
    // `$fetch` rejects with the response body on `data`, which is where the
    // endpoint says what the chain or the policy objected to.
    sponsorSettle.mockRejectedValueOnce({
      data: { error: 'The sponsored payment was not accepted: custom program error: 0x1' }
    })

    const checkout = checkoutFor(quoteFor(false))
    await checkout.pay()

    expect(checkout.failure.value).toBe(
      'The sponsored payment was not accepted: custom program error: 0x1'
    )
  })
})

describe('a token price the wallet has to pay', () => {
  it('refuses a price it does not hold, before the wallet opens', async () => {
    walletTokens.value = 0

    const checkout = checkoutFor(quoteFor(false))
    await checkout.pay()

    expect(checkout.failure.value).toBe('errors.walletShortOnPrice')
    expect(buildInstructionMock).not.toHaveBeenCalled()
    expect(signTransactions).not.toHaveBeenCalled()
    expect(sponsorSettle).not.toHaveBeenCalled()
  })
  it('treats a token account that does not exist as holding nothing', async () => {
    walletTokenAccount.exists = false

    const checkout = checkoutFor(quoteFor(false))
    await checkout.pay()

    expect(checkout.failure.value).toBe('errors.walletShortOnPrice')
    expect(signTransactions).not.toHaveBeenCalled()
  })

  it('does not call a wallet short on the price when the cluster did not answer', async () => {
    walletTokenAccount.unreachable = true

    const checkout = checkoutFor(quoteFor(false))
    await checkout.pay()

    // The buyer is told the read failed, not sent off to add funds they have.
    expect(checkout.failure.value).toBe('errors.balanceUnreadable')
    expect(signTransactions).not.toHaveBeenCalled()
    expect(sponsorSettle).not.toHaveBeenCalled()
  })
})
