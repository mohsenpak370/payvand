import { address } from '@vue-solana/nuxt/kit'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { invoiceCosts, type InvoiceCosts } from '~/utils/invoiceCost'
import { usePaymentCheckout } from './usePaymentCheckout'

/**
 * `pay()` is the last thing between an invoice and a signature, and one of its
 * preflight checks exists for a state no button can reach: the treasury has no
 * account for the mint, so the program's fee has nowhere to go and the settle is
 * rejected — after the buyer signed. The page holds the button for that, so the
 * test calls `pay()` directly, the way the stage machine would. If the guard
 * goes away, `pay()` builds and sends the transaction and this fails.
 *
 * The composable leans on Nuxt's auto-imports; they are stubbed as globals here,
 * which is enough of the app to reach the guard and a wallet that would be asked
 * to sign if the payment got that far.
 */

// Just enough reactivity for a test that only reads: `vue` is not a direct
// dependency of this package, and nothing here needs effects or caching.
const ref = <T>(value: T) => ({ value })
const computed = <T>(compute: () => T) => ({
  get value(): T {
    return compute()
  }
})
const watch = (..._args: unknown[]) => {}

const seller = address('6dQgFZDTkhDMTZv9ftZe4ufU9jG5fvJEw6zpw6WCKjPz')
const payer = address('3rpBzUBbZPXFKhGywCRduD3hrREDjvDtaw2Mos2yUUPx')
const mint = address('4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU')
const reference = address('A8yxpWod2xrPd6YbXnaKWCTkk2WduzQKdKPUANZFQpft')

const execute = vi.fn(async () => 'SIGNATURE')
const sendSilentTokenPayment = vi.fn(async () => 'SILENT_SIGNATURE')
const buildInstruction = vi.fn(() => ({
  programAddress: reference,
  accounts: [],
  data: new Uint8Array()
}))

/** A 1.5 USDC invoice, priced the way the program prices one. */
function quoteFor(treasuryTokenAccountMissing: boolean): InvoiceCosts {
  return invoiceCosts({
    policy: { feeBps: 200, fixedFeeLamports: 0, feeMode: 'seller' },
    amount: 1_500_000,
    currency: 'usdc',
    invoiceRent: 1_000_000,
    receiptRent: 1_000_000,
    treasuryTokenAccountMissing
  })
}

function checkoutFor(quote: InvoiceCosts) {
  const checkout = usePaymentCheckout()
  checkout.payload.value = {
    v: 1,
    id: reference,
    seller,
    reference,
    amount_lamports: 0,
    options: [{ currency: 'usdc', mint, decimals: 6, amount: 1_500_000, payment_pda: reference }],
    memo: '',
    name: 'Treasury preflight',
    cluster: 'devnet',
    endpoint: 'https://api.devnet.solana.com'
  }
  checkout.costs.value = quote
  return checkout
}

beforeEach(() => {
  execute.mockClear()
  buildInstruction.mockClear()
  vi.stubGlobal('ref', ref)
  vi.stubGlobal('computed', computed)
  vi.stubGlobal('watch', watch)
  vi.stubGlobal('onBeforeUnmount', () => {})
  vi.stubGlobal('appT', (key: string) => key)
  vi.stubGlobal('buildInstruction', buildInstruction)
  vi.stubGlobal('compileWireTransaction', async () => ({}))
  vi.stubGlobal('usePaymentApi', () => ({
    getPayment: async () => null,
    validateDiscount: async () => null
  }))
  vi.stubGlobal('useSolanaClient', () => ({ client: { rpc: {} } }))
  vi.stubGlobal('useSolanaWallet', () => ({
    publicKey: ref(payer),
    connected: ref(true),
    canSignTransaction: ref(true)
  }))
  vi.stubGlobal('useSolanaSignAndSendTransaction', () => ({ execute }))
  sendSilentTokenPayment.mockClear()
  vi.stubGlobal('useBudget', () => ({ sendSilentTokenPayment }))
  vi.stubGlobal('useInvoiceCosts', () => ({ estimateInvoiceCosts: async () => null }))
})

afterEach(() => {
  vi.unstubAllGlobals()
})

/** The same invoice with a USDC allowance this device can sign for. */
function withAllowance(checkout: ReturnType<typeof usePaymentCheckout>) {
  checkout.tokenBudgetState.value = {
    pda: reference,
    buyer: payer,
    sessionKey: payer,
    mint,
    monthlyLimit: 5_000_000,
    spent: 0,
    monthIndex: 1,
    lamports: 2_000_000,
    amount: 5_000_000
  }
  checkout.sessionReady.value = true
  checkout.record.value = {
    id: reference,
    status: 'pending',
    payment_pda: reference,
    seller,
    reference,
    amount_lamports: 0,
    memo: '',
    name: '',
    description: '',
    cluster: 'devnet',
    endpoint: 'https://api.devnet.solana.com',
    seller_user: null,
    settled_by: null,
    created_at_unix: 0,
    settled_at_unix: 0
  }
  return checkout
}

describe('pay()', () => {
  it('refuses before building anything when the treasury has no account for the mint', async () => {
    const checkout = checkoutFor(quoteFor(true))

    await checkout.pay()

    expect(checkout.failure.value).toBe('errors.treasuryTokenAccountMissing')
    expect(buildInstruction).not.toHaveBeenCalled()
    expect(execute).not.toHaveBeenCalled()
    expect(checkout.signature.value).toBeNull()
  })

  it('builds and sends the same invoice once that account is there', async () => {
    const checkout = checkoutFor(quoteFor(false))

    await checkout.pay()

    expect(checkout.failure.value).toBe('')
    expect(buildInstruction).toHaveBeenCalledWith(
      'settle_token_payment',
      expect.anything(),
      expect.anything()
    )
    expect(execute).toHaveBeenCalledTimes(1)
    expect(checkout.signature.value).toBe('SIGNATURE')
  })
})

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
