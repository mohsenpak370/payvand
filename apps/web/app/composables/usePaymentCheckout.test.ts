import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import {
  buildInstructionMock,
  checkoutFor,
  checkoutForSol,
  compileWireTransactionMock,
  eurcMint,
  execute,
  getSponsor,
  mint,
  payer,
  preparedSettleMatchesMock,
  preparedWire,
  quoteFor,
  reference,
  relayerUnavailable,
  seller,
  setupCheckout,
  signTransactions,
  sponsor,
  sponsorPrepare,
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
 * The other subject here is who pays the settle's lamports, which is the rule
 * in `docs/payment-costs.md`: the platform does, on every wallet payment, SOL
 * or token. The relayer builds and signs the settle first, the page checks it
 * is this invoice's, the wallet signs, the API sends. The wallet pays its own
 * way only when no relayer will — and then is refunded in the same transaction.
 */

beforeEach(setupCheckout)
afterEach(teardownCheckout)

describe('pay()', () => {
  it('refuses before building anything when the treasury has no account for the mint', async () => {
    const checkout = checkoutFor(quoteFor(true))

    await checkout.pay()

    expect(checkout.failure.value).toBe('errors.treasuryTokenAccountMissing')
    expect(buildInstructionMock).not.toHaveBeenCalled()
    expect(sponsorPrepare).not.toHaveBeenCalled()
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
    expect(sponsorSettle).toHaveBeenCalledTimes(1)
    expect(checkout.signature.value).toBe('SPONSORED_SIGNATURE')
  })
})

/**
 * The invariant: while a relayer is available, it is the fee payer and the
 * sponsor of every wallet payment, however much SOL the wallet holds. A funded
 * wallet paying its own way would have the buyer front what the platform pays.
 */
describe('the platform pays the settle', () => {
  it.each([
    ['usdc', mint],
    ['eurc', eurcMint]
  ] as const)('hands a %s invoice to the relayer even when the wallet holds plenty of SOL', async (currency, tokenMint) => {
    walletLamports.value = 10_000_000_000
    const checkout = checkoutFor(quoteFor(false, currency, tokenMint), currency, tokenMint)

    await checkout.pay()

    expect(sponsorPrepare).toHaveBeenCalledTimes(1)
    expect(sponsorPrepare.mock.calls[0]![0]).toMatchObject({
      buyer: payer,
      seller,
      reference,
      mint: tokenMint,
      amount: 1_500_000
    })
    // The page's own copy of the settle names the relayer as sponsor, and the
    // prepared bytes are checked against it before the wallet sees them.
    expect(buildInstructionMock).toHaveBeenCalledWith(
      'settle_token_payment',
      expect.anything(),
      expect.objectContaining({ sponsor, mint: tokenMint })
    )
    expect(preparedSettleMatchesMock).toHaveBeenCalledWith(preparedWire, sponsor, expect.anything())
    // The wallet signs the relayer-signed bytes and sends nothing itself.
    expect(signTransactions).toHaveBeenCalledWith([preparedWire])
    expect(execute).not.toHaveBeenCalled()
    expect(sponsorSettle.mock.calls[0]![0]).toMatchObject({
      transaction: 'BASE64_WIRE',
      buyer: payer,
      mint: tokenMint
    })
    expect(checkout.signature.value).toBe('SPONSORED_SIGNATURE')
    expect(checkout.failure.value).toBe('')
  })

  it('hands a SOL invoice to the relayer even when the wallet could front it', async () => {
    const price = 1_000_000_000
    walletLamports.value = price + 1_000_000_000
    const checkout = checkoutForSol(price)

    await checkout.pay()

    expect(sponsorPrepare.mock.calls[0]![0]).toMatchObject({ buyer: payer, mint: '', amount: price })
    expect(buildInstructionMock).toHaveBeenCalledWith(
      'settle_payment',
      expect.objectContaining({ amount: price }),
      expect.objectContaining({ sponsor })
    )
    expect(execute).not.toHaveBeenCalled()
    expect(checkout.signature.value).toBe('SPONSORED_SIGNATURE')
  })

  it('lets a buyer holding no SOL at all pay a token invoice', async () => {
    walletLamports.value = 0
    const checkout = checkoutFor(quoteFor(false))

    await checkout.pay()

    expect(checkout.signature.value).toBe('SPONSORED_SIGNATURE')
    expect(checkout.failure.value).toBe('')
  })

  it('never opens the wallet on a prepared transaction that is not this invoice', async () => {
    preparedSettleMatchesMock.mockImplementation(() => false)
    const checkout = checkoutFor(quoteFor(false))

    await checkout.pay()

    expect(checkout.failure.value).toBe('errors.preparedMismatch')
    expect(signTransactions).not.toHaveBeenCalled()
    expect(sponsorSettle).not.toHaveBeenCalled()
    expect(execute).not.toHaveBeenCalled()
  })
})

/**
 * Only when no relayer will pay does the wallet pay its own way, as its own
 * sponsor. The program refunds it both rents and its signature, and the
 * compute-unit limit keeps the wallet from adding a priority price nothing
 * refunds — so the buyer still spends exactly the price.
 */
describe('when no relayer will pay', () => {
  it('lets a funded wallet pay its own way behind a compute-unit limit', async () => {
    getSponsor.mockImplementation(async () => relayerUnavailable())
    const checkout = checkoutFor(quoteFor(false))

    await checkout.pay()

    expect(sponsorPrepare).not.toHaveBeenCalled()
    expect(execute).toHaveBeenCalledTimes(1)
    expect(checkout.signature.value).toBe('SIGNATURE')
    expect(buildInstructionMock).toHaveBeenCalledWith(
      'settle_token_payment',
      expect.anything(),
      expect.objectContaining({ sponsor: payer })
    )
    const compiled = compileWireTransactionMock.mock.calls[0]![0]
    expect(compiled.payer).toBe(payer)
    const [limit] = compiled.instruction as { programAddress: string, data: Uint8Array }[]
    expect(limit!.programAddress).toBe('ComputeBudget111111111111111111111111111111')
    expect(limit!.data[0]).toBe(2)
  })

  it('refuses before a signature when the wallet cannot front the records either', async () => {
    getSponsor.mockImplementation(async () => relayerUnavailable())
    walletLamports.value = 0

    const checkout = checkoutFor(quoteFor(false))
    await checkout.pay()

    // Nothing can front the fee, so the payment stops here and says why
    // instead of failing inside a wallet prompt the buyer already approved.
    expect(checkout.failure.value).toBe('errors.paymentNeedsSol')
    expect(checkout.signature.value).toBeNull()
    expect(execute).not.toHaveBeenCalled()
    expect(signTransactions).not.toHaveBeenCalled()
  })

  it('treats a relayer request that failed as no relayer', async () => {
    getSponsor.mockRejectedValue(new Error('offline'))
    const checkout = checkoutFor(quoteFor(false))

    await checkout.pay()

    expect(sponsorPrepare).not.toHaveBeenCalled()
    expect(checkout.signature.value).toBe('SIGNATURE')
  })
})

describe('a SOL price the wallet has to pay', () => {
  it('refuses a price the wallet does not hold, before the wallet opens', async () => {
    walletLamports.value = 5_500_000

    const checkout = checkoutForSol(1_000_000_000)
    await checkout.pay()

    // A relayer covers the fee and the rents, never the price, so nothing can
    // be built: the buyer is told what the invoice costs instead.
    expect(checkout.failure.value).toBe('errors.walletShortOnPrice')
    expect(buildInstructionMock).not.toHaveBeenCalled()
    expect(sponsorPrepare).not.toHaveBeenCalled()
    expect(execute).not.toHaveBeenCalled()
    expect(signTransactions).not.toHaveBeenCalled()
  })
})

describe('a send the API refuses', () => {
  it('shows the API reason instead of the bare status', async () => {
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
