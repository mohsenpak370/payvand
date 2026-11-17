import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  checkoutFor,
  execute,
  quoteFor,
  reference,
  sendSilentTokenPayment,
  setupCheckout,
  teardownCheckout,
  tokenFloatTopUpInstructions,
  wallet,
  withAllowance
} from '../test-utils/usePaymentCheckout'

beforeEach(setupCheckout)
afterEach(teardownCheckout)

/**
 * The allowance is the one path that sends with no wallet prompt, so the hold is
 * the only thing between a stray tap — or an old version of this page that paid
 * on load — and someone's money. `pay(true)` is what a completed hold passes.
 */
describe('paying from the allowance', () => {
  it('sends nothing without the hold', async () => {
    const checkout = withAllowance(checkoutFor(quoteFor(false)))

    await checkout.pay()

    expect(sendSilentTokenPayment).not.toHaveBeenCalled()
    expect(checkout.signature.value).toBeNull()
    expect(checkout.failure.value).toBe('')
  })

  it('pays from the allowance when the wallet cannot sign', async () => {
    // The session key is what signs this payment; the wallet only says who is
    // paying. Gating the allowance on the wallet's own signer took the hold
    // button away from wallets that report they cannot sign.
    wallet.canSign = false
    const checkout = withAllowance(checkoutFor(quoteFor(false)))
    // `load()` leaves this false once the code is read; these tests set the
    // payload directly.
    checkout.loading.value = false

    expect(checkout.stage.value).toBe('silent')
    expect(checkout.canPayFromAllowance.value).toBe(true)
    expect(checkout.canPayWithWallet.value).toBe(false)

    checkout.beginHold()
    expect(checkout.holding.value).toBe(true)
    checkout.stopHold()

    await checkout.pay(true)
    expect(sendSilentTokenPayment).toHaveBeenCalledTimes(1)
    expect(checkout.signature.value).toBe('SILENT_SIGNATURE')
  })

  it('says plainly when neither path can pay', async () => {
    wallet.canSign = false
    // No allowance here, so nothing can sign this invoice.
    const checkout = checkoutFor(quoteFor(false))
    checkout.loading.value = false

    expect(checkout.stage.value).toBe('wallet')
    expect(checkout.canPayFromAllowance.value).toBe(false)
    expect(checkout.canPayWithWallet.value).toBe(false)
    expect(checkout.blockedReason.value).toBe('checkout.walletCannotSign')
  })

  it('keeps the device’s own button when the invoice is what cannot be paid', () => {
    // An allowance this device can sign for, on an invoice whose treasury account
    // the seller never opened: the stage is still the allowance's, because the
    // device does have a way to pay, and `invoicePayable` is what keeps the
    // button disabled — the page says why, above it. Sending the buyer to the
    // wallet stage would offer a second button that cannot pay either.
    const checkout = withAllowance(checkoutFor(quoteFor(true)))
    checkout.loading.value = false

    // The change itself: the device still has a way to pay, and neither path may
    // take this invoice. Without `canPaySilently` the stage name alone could be
    // produced by conditions that have nothing to do with the treasury account.
    expect(checkout.canPaySilently.value).toBe(true)
    expect(checkout.canPayFromAllowance.value).toBe(false)
    expect(checkout.canPayWithWallet.value).toBe(false)
    expect(checkout.stage.value).toBe('silent')
  })

  it('keeps the button away until the record catches up with the send', async () => {
    const checkout = withAllowance(checkoutFor(quoteFor(false)))
    checkout.loading.value = false
    // What the API still says right after a confirmed send: unpaid, because the
    // receipt has not been indexed yet. This is the window where a second press
    // used to end in a chain error.
    const pending = checkout.record.value!
    vi.stubGlobal('usePaymentApi', () => ({
      getPayment: async () => pending,
      validateDiscount: async () => null
    }))

    await checkout.pay(true)

    expect(sendSilentTokenPayment).toHaveBeenCalledTimes(1)
    expect(checkout.stage.value).toBe('confirming')
    expect(checkout.canPayFromAllowance.value).toBe(false)
    expect(checkout.canPayWithWallet.value).toBe(false)

    // Out of reach in the composable too, not only on the page: a press builds
    // nothing, and a stray `pay` sends nothing.
    checkout.beginHold()
    expect(checkout.holding.value).toBe(false)
    await checkout.pay(true)
    expect(sendSilentTokenPayment).toHaveBeenCalledTimes(1)

    // What the poll finds: the receipt the send is waiting for.
    checkout.record.value = { ...pending, status: 'settled' }
    expect(checkout.stage.value).toBe('settled')
  })

  it('refills the float before paying when it is spent', async () => {
    const checkout = withAllowance(checkoutFor(quoteFor(false)))
    // What a spent float needs: the buyer's transfers, which the wallet signs.
    tokenFloatTopUpInstructions.mockResolvedValueOnce([
      { programAddress: reference, accounts: [], data: new Uint8Array() }
    ])

    await checkout.pay(true)

    expect(execute).toHaveBeenCalledTimes(1)
    expect(execute.mock.invocationCallOrder[0]).toBeLessThan(
      sendSilentTokenPayment.mock.invocationCallOrder[0]!
    )
    expect(sendSilentTokenPayment).toHaveBeenCalledTimes(1)
    expect(checkout.signature.value).toBe('SILENT_SIGNATURE')
  })

  it('pays from the allowance once the hold is confirmed', async () => {
    const checkout = withAllowance(checkoutFor(quoteFor(false)))

    await checkout.pay(true)

    expect(sendSilentTokenPayment).toHaveBeenCalledTimes(1)
    expect(checkout.signature.value).toBe('SILENT_SIGNATURE')
  })
})

/**
 * The hold is what lets a payment leave without a wallet prompt, so both halves
 * matter: it must not fire early, and a released press must never fire later.
 *
 * The clock is faked so the three seconds pass without the test waiting, but a
 * faked clock only advances timers: the payment's own awaits (address
 * derivation, the stubbed RPC) settle on a real turn, so each step flushes a few
 * of those before it asserts.
 */
const realSetTimeout = globalThis.setTimeout

function realTurn() {
  return new Promise<void>((resolve) => {
    realSetTimeout(resolve, 0)
  })
}

async function flushRealTurns() {
  for (let turn = 0; turn < 20; turn += 1) {
    await realTurn()
  }
}

describe('the hold on the allowance button', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('pays only once the hold has lasted the whole count', async () => {
    const checkout = withAllowance(checkoutFor(quoteFor(false)))

    checkout.beginHold()
    await vi.advanceTimersByTimeAsync(2_900)
    await flushRealTurns()
    expect(sendSilentTokenPayment).not.toHaveBeenCalled()
    expect(checkout.signature.value).toBeNull()
    expect(checkout.holding.value).toBe(true)

    await vi.advanceTimersByTimeAsync(100)
    await flushRealTurns()
    expect(sendSilentTokenPayment).toHaveBeenCalledTimes(1)
    expect(checkout.signature.value).toBe('SILENT_SIGNATURE')
    expect(checkout.holding.value).toBe(false)
  })

  it('keeps one count when the press repeats', async () => {
    const checkout = withAllowance(checkoutFor(quoteFor(false)))

    checkout.beginHold()
    await vi.advanceTimersByTimeAsync(2_900)
    // What a held space bar does: another keydown every few ticks. A press that
    // restarted the clock would drop the count back to zero here, and one that
    // started a second clock would leave a timer ticking past the payment,
    // which nothing would ever clear.
    checkout.beginHold()
    expect(checkout.holdProgress.value).toBeGreaterThan(0.9)
    expect(vi.getTimerCount()).toBe(1)

    await vi.advanceTimersByTimeAsync(100)
    await flushRealTurns()
    expect(sendSilentTokenPayment).toHaveBeenCalledTimes(1)
    expect(vi.getTimerCount()).toBe(0)
  })

  it('sends nothing when the hold is released early', async () => {
    const checkout = withAllowance(checkoutFor(quoteFor(false)))

    checkout.beginHold()
    await vi.advanceTimersByTimeAsync(2_900)
    checkout.stopHold()
    expect(checkout.holding.value).toBe(false)
    expect(checkout.holdProgress.value).toBe(0)

    // And nothing afterwards either: a stopped hold leaves no timer behind.
    await vi.advanceTimersByTimeAsync(10_000)
    await flushRealTurns()
    expect(sendSilentTokenPayment).not.toHaveBeenCalled()
    expect(checkout.signature.value).toBeNull()
    expect(checkout.failure.value).toBe('')
  })
})
