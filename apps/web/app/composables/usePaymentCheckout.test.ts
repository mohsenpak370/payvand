import { address } from '@vue-solana/nuxt/kit'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { invoiceCosts, type InvoiceCosts } from '~/utils/invoiceCost'
import { usePaymentCheckout } from './usePaymentCheckout'
import { derivePaymentPda, derivePaymentReceiptPda } from './usePaymentProgram'

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
/** The transfers a spent float needs; most payments here need none. */
const tokenFloatTopUpInstructions = vi.fn(async () => [] as unknown[])
/** What the wallet reports about its own signer, per test. */
let walletCanSign = true
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
  walletCanSign = true
  vi.stubGlobal('useSolanaWallet', () => ({
    publicKey: ref(payer),
    connected: ref(true),
    canSignTransaction: ref(walletCanSign)
  }))
  vi.stubGlobal('useSolanaSignAndSendTransaction', () => ({ execute }))
  sendSilentTokenPayment.mockClear()
  tokenFloatTopUpInstructions.mockClear()
  tokenFloatTopUpInstructions.mockImplementation(async () => [])
  vi.stubGlobal('useBudget', () => ({ sendSilentTokenPayment, tokenFloatTopUpInstructions }))
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
 * A reload lands in the gap between a confirmed send and the record that reports
 * it: the API's node answers at its own commitment, so for a few seconds it still
 * says `pending`. The page trusted the record alone, so the button came back for
 * an invoice the chain had already paid, and the second press earned the chain
 * error that made the buyer reload in the first place. `load()` asks the chain.
 */
describe('a page that loads with the payment already on chain', () => {
  const amount = 250_000
  /** What the API answered on its own, before the chain was consulted. */
  let answered: string[] = []

  /** A record still catching up: what the API answers right after a payment. */
  function pendingRecord(id: string) {
    return {
      id,
      status: 'pending' as const,
      payment_pda: id,
      seller,
      reference,
      amount_lamports: amount,
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
  }

  /**
   * The payer the API reports, which a test can move while the page is open: the
   * composable takes its api at setup, the same way it takes its client.
   */
  let apiPayer: string | null = null

  /**
   * The accounts the buyer's node holds, and how often it was asked. The
   * composable takes its client at setup, so the stub reads this list on every
   * call: a test can move the chain under a page that is already open, and can
   * count the reads a settled page must stop making.
   */
  let chainAccounts: string[] = []
  let chainReads = 0

  /** The buyer's node, holding exactly the accounts named. */
  function chainHolds(accounts: string[]) {
    chainAccounts = accounts
    vi.stubGlobal('useSolanaClient', () => ({
      client: {
        rpc: {
          getAccountInfo: (key: string) => ({
            send: async () => {
              chainReads += 1
              return { value: chainAccounts.includes(key) ? { data: ['', 'base64'] } : null }
            }
          })
        }
      }
    }))
  }

  async function loadInvoice(invoice: string) {
    vi.stubGlobal('unwrapQrText', (text: string) => text)
    vi.stubGlobal('parseQrPayload', () => ({
      v: 1,
      id: invoice,
      seller,
      reference,
      amount_lamports: amount,
      memo: '',
      name: 'Reloaded invoice',
      cluster: 'devnet',
      endpoint: 'https://api.devnet.solana.com'
    }))
    vi.stubGlobal('usePaymentApi', () => ({
      getPayment: async (id: string) => {
        const fetched = { ...pendingRecord(id), settled_by: apiPayer }
        answered.push(fetched.status)
        return fetched
      },
      validateDiscount: async () => null
    }))
    const checkout = usePaymentCheckout()
    await checkout.load('payvand://reloaded')
    return checkout
  }

  beforeEach(() => {
    answered = []
    apiPayer = null
    chainAccounts = []
    chainReads = 0
    // `load()` starts the record poll; the clock is faked so it never fires here.
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('offers no button when the receipt exists and the record has not caught up', async () => {
    const invoice = await derivePaymentPda(seller, reference, amount)
    // Every `settle_*` closes its invoice in the same transaction, so a settled
    // invoice leaves only its receipt — this is the account the page must read,
    // and the test fails if it reads the invoice account instead.
    chainHolds([(await derivePaymentReceiptPda(invoice)).toString()])

    const checkout = await loadInvoice(invoice)

    // The API lagged; the chain did not, and the record carries the answer so
    // every other decision on the page follows from it.
    expect(answered).toEqual(['pending'])
    expect(checkout.record.value?.status).toBe('settled')
    expect(checkout.stage.value).toBe('settled')
    expect(checkout.canPayFromAllowance.value).toBe(false)
    expect(checkout.canPayWithWallet.value).toBe(false)
    // Out of reach in the composable too, for both paths: no hold starts, and a
    // press that reached `pay()` builds nothing.
    checkout.beginHold()
    expect(checkout.holding.value).toBe(false)
    await checkout.pay(true)
    await checkout.pay()
    expect(sendSilentTokenPayment).not.toHaveBeenCalled()
    expect(execute).not.toHaveBeenCalled()
    expect(checkout.signature.value).toBeNull()
  })

  it('still offers the invoice when the chain holds nothing for it', async () => {
    const invoice = await derivePaymentPda(seller, reference, amount)
    chainHolds([])

    const checkout = await loadInvoice(invoice)

    expect(checkout.record.value?.status).toBe('pending')
    expect(checkout.stage.value).toBe('wallet')
    expect(checkout.canPayWithWallet.value).toBe(true)
  })

  it('settles the open page once the invoice is paid somewhere else', async () => {
    const invoice = await derivePaymentPda(seller, reference, amount)
    chainHolds([])
    const checkout = await loadInvoice(invoice)
    // Left open and unpaid: the buyer's own wallet is the action.
    expect(checkout.stage.value).toBe('wallet')

    // Paid from another wallet, or another device: the chain holds the receipt
    // while the API's node still answers `pending`.
    chainHolds([(await derivePaymentReceiptPda(invoice)).toString()])
    await vi.advanceTimersByTimeAsync(5_000)

    // The tick reads the API and then the chain, so the answer lands a couple of
    // awaits after the timer fires rather than on it.
    await vi.waitFor(() => expect(checkout.record.value?.status).toBe('settled'))
    expect(checkout.stage.value).toBe('settled')
    expect(checkout.canPayWithWallet.value).toBe(false)
    expect(checkout.canPayFromAllowance.value).toBe(false)
  })

  it('asks the chain less often than the API while the invoice stays unpaid', async () => {
    const invoice = await derivePaymentPda(seller, reference, amount)
    chainHolds([])
    const checkout = await loadInvoice(invoice)
    expect(checkout.stage.value).toBe('wallet')

    const atLoad = chainReads
    // The first tick asks the chain, so a payment made elsewhere is noticed at
    // once rather than only when the indexer catches up.
    await vi.advanceTimersByTimeAsync(5_000)
    await vi.waitFor(() => expect(chainReads).toBe(atLoad + 1))

    // A minute of an unpaid page: twelve API ticks, and the chain asked a
    // handful of times — the gap widens as nothing moves.
    await vi.advanceTimersByTimeAsync(55_000)
    await vi.waitFor(() => expect(answered.length).toBe(13))
    const reads = chainReads - atLoad
    expect(reads).toBeGreaterThanOrEqual(2)
    expect(reads).toBeLessThanOrEqual(4)

    // And the widest gap still notices: the next payment shows up within a
    // minute of it landing, without a reload.
    chainHolds([(await derivePaymentReceiptPda(invoice)).toString()])
    await vi.advanceTimersByTimeAsync(60_000)
    await vi.waitFor(() => expect(checkout.stage.value).toBe('settled'))
  })

  it('stops reading a settled record, API and chain alike', async () => {
    const invoice = await derivePaymentPda(seller, reference, amount)
    chainHolds([(await derivePaymentReceiptPda(invoice)).toString()])
    // The API has the payer, so this record settles with nothing left to ask.
    apiPayer = 'WalletPaidFromAnotherDevice111111111111111111'
    const checkout = await loadInvoice(invoice)
    expect(checkout.stage.value).toBe('settled')
    expect(checkout.record.value?.settled_by).toBe(apiPayer)

    // A paid page can sit open for as long as the buyer leaves the tab: several
    // intervals later it must have asked neither the API nor the chain again.
    const asked = answered.length
    const reads = chainReads
    await vi.advanceTimersByTimeAsync(30_000)

    expect(answered.length).toBe(asked)
    expect(chainReads).toBe(reads)
  })

  it('names the payer once the API catches up with a payment the chain already held', async () => {
    const invoice = await derivePaymentPda(seller, reference, amount)
    chainHolds([(await derivePaymentReceiptPda(invoice)).toString()])
    const checkout = await loadInvoice(invoice)
    // Settled from the chain alone: the sheet can say paid but not yet who paid.
    expect(checkout.stage.value).toBe('settled')
    expect(checkout.record.value?.settled_by).toBeNull()

    // The API's node still calls it pending, but it has the payer now.
    apiPayer = 'WalletPaidFromAnotherDevice111111111111111111'
    await vi.advanceTimersByTimeAsync(5_000)
    await vi.waitFor(() => expect(checkout.record.value?.settled_by).toBe(apiPayer))
    // The payer is taken; the status is not, because the chain already settled it.
    expect(checkout.record.value?.status).toBe('settled')

    // Nothing left to ask: the poll stops rather than calling the API forever.
    const asked = answered.length
    await vi.advanceTimersByTimeAsync(15_000)
    expect(answered.length).toBe(asked)
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

  it('pays from the allowance when the wallet cannot sign', async () => {
    // The session key is what signs this payment; the wallet only says who is
    // paying. Gating the allowance on the wallet's own signer took the hold
    // button away from wallets that report they cannot sign.
    walletCanSign = false
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
    walletCanSign = false
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
