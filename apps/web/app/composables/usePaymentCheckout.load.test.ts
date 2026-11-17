import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { PaymentOption } from './usePaymentApi'
import { usePaymentCheckout } from './usePaymentCheckout'
import {
  execute,
  mint,
  reference,
  seller,
  sendSilentTokenPayment,
  setupCheckout,
  teardownCheckout
} from '../test-utils/usePaymentCheckout'
import {
  derivePaymentPda,
  derivePaymentReceiptPda,
  deriveTokenPaymentPda
} from './usePaymentProgram'

beforeEach(setupCheckout)
afterEach(teardownCheckout)

/**
 * A reload can land in the slot between a send the cluster has confirmed and the
 * API's node voting it `confirmed`: for that moment the record still says
 * `pending`. The page trusted the record alone, so the button came back for an
 * invoice the chain had already paid, and the second press earned the chain error
 * that made the buyer reload in the first place. `load()` asks the chain once;
 * the poll takes its answers from the API, which now reads at `confirmed` too.
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

  /** Whether the API has caught up with the payment, which it now does in a slot. */
  let apiSettled = false

  /**
   * The accounts the buyer's node holds, and how often it was asked. The
   * composable takes its client at setup, so the stub reads this list on every
   * call: a test can move the chain under a page that is already open, and can
   * count the reads a settled page must stop making.
   */
  let chainAccounts: string[] = []
  let chainReads = 0
  /** Every account the node was asked for, at the moment it was asked. */
  let chainAsked: string[] = []

  /** The buyer's node, holding exactly the accounts named. */
  function chainHolds(accounts: string[]) {
    chainAccounts = accounts
    vi.stubGlobal('useSolanaClient', () => ({
      client: {
        rpc: {
          getAccountInfo: (key: string) => {
            // Recorded when the read is asked for, not when it lands: the schedule
            // decides at that moment, and a slow node must not look like a later
            // read arriving.
            chainAsked.push(key)
            return {
              send: async () => {
                chainReads += 1
                return { value: chainAccounts.includes(key) ? { data: ['', 'base64'] } : null }
              }
            }
          }
        }
      }
    }))
  }

  async function loadInvoice(invoice: string, options?: PaymentOption[]) {
    vi.stubGlobal('unwrapQrText', (text: string) => text)
    vi.stubGlobal('parseQrPayload', () => ({
      v: 1,
      id: invoice,
      seller,
      reference,
      amount_lamports: amount,
      ...(options ? { options } : {}),
      memo: '',
      name: 'Reloaded invoice',
      cluster: 'devnet',
      endpoint: 'https://api.devnet.solana.com'
    }))
    vi.stubGlobal('usePaymentApi', () => ({
      getPayment: async (id: string) => {
        const fetched = {
          ...pendingRecord(id),
          status: (apiSettled ? 'settled' : 'pending') as 'settled' | 'pending',
          settled_by: apiPayer
        }
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
    apiSettled = false
    chainAccounts = []
    chainReads = 0
    chainAsked = []
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

  it('settles the open page from an API tick, and asks the chain nothing more', async () => {
    const invoice = await derivePaymentPda(seller, reference, amount)
    chainHolds([])
    const checkout = await loadInvoice(invoice)
    // Left open and unpaid: the buyer's own wallet is the action.
    expect(checkout.stage.value).toBe('wallet')

    // Paid from another wallet, or another device. The API reads the cluster at
    // `confirmed`, the level the send waited for, so the very next tick has it —
    // and the page does not have to ask the buyer's RPC anything to find out.
    apiSettled = true
    await vi.advanceTimersByTimeAsync(5_000)
    await vi.waitFor(() => expect(checkout.stage.value).toBe('settled'))

    expect(checkout.canPayWithWallet.value).toBe(false)
    expect(checkout.canPayFromAllowance.value).toBe(false)
    // Nothing on the chain, so this settle is the API's own answer.
    expect(answered).toEqual(['pending', 'settled'])
  })

  it('keeps a chain-settled page off the chain while the API still lags', async () => {
    const invoice = await derivePaymentPda(seller, reference, amount)
    chainHolds([(await derivePaymentReceiptPda(invoice)).toString()])
    const checkout = await loadInvoice(invoice)
    // Settled from the chain alone, without a payer: the sheet says paid, and the
    // poll keeps asking the API who paid.
    expect(checkout.stage.value).toBe('settled')
    expect(checkout.record.value?.settled_by).toBeNull()
    const readsAtLoad = chainReads

    // The API never catches up in this test, so the poll runs the whole minute —
    // twelve more API ticks. The chain has already answered this page; asking it
    // again every few ticks would buy nothing.
    await vi.advanceTimersByTimeAsync(60_000)
    await vi.waitFor(() => expect(answered.length).toBe(13))
    expect(chainReads).toBe(readsAtLoad)
  })

  it('reads the chain on a schedule that doubles, while the API keeps to every tick', async () => {
    const invoice = await derivePaymentPda(seller, reference, amount)
    chainHolds([])
    const checkout = await loadInvoice(invoice)
    expect(checkout.stage.value).toBe('wallet')
    const readsAtLoad = chainReads
    const askedAtLoad = answered.length

    // A minute of an unpaid page: twelve API ticks — the poll unchanged — and a
    // chain read on the first, the third and the seventh of them, because the
    // wait after each read doubles.
    await vi.advanceTimersByTimeAsync(60_000)
    await vi.waitFor(() => expect(answered.length).toBe(askedAtLoad + 12))
    // A read asked for on the twelfth tick may still be landing; the schedule is
    // what is being counted, not the node.
    await vi.waitFor(() => expect(chainReads - readsAtLoad).toBe(3))
  })

  it('tops the gap out at a minute, so a long-open page still asks now and then', async () => {
    const invoice = await derivePaymentPda(seller, reference, amount)
    chainHolds([])
    await loadInvoice(invoice)
    const readsAtLoad = chainReads
    const askedAtLoad = answered.length

    // Ten minutes: 120 API ticks, and a chain read at 5 s, 15 s and 35 s, then one
    // a minute — twelve in all, rather than three every five seconds for an hour.
    await vi.advanceTimersByTimeAsync(600_000)
    await vi.waitFor(() => expect(answered.length).toBe(askedAtLoad + 120))
    await vi.waitFor(() => expect(chainReads - readsAtLoad).toBe(12))
  })

  it('starts the chain schedule over when the buyer switches currency', async () => {
    const solPda = await derivePaymentPda(seller, reference, amount)
    const tokenAmount = 1_500_000
    const usdcPda = await deriveTokenPaymentPda(seller, reference, mint, tokenAmount)
    chainHolds([])
    const checkout = await loadInvoice(solPda, [
      { currency: 'sol', mint: '', decimals: 9, amount, payment_pda: solPda },
      { currency: 'usdc', mint, decimals: 6, amount: tokenAmount, payment_pda: usdcPda }
    ])
    expect(checkout.selectedCurrency.value).toBe('sol')

    // A minute unpaid: the schedule has read three times and is waiting a minute.
    await vi.advanceTimersByTimeAsync(60_000)
    await vi.waitFor(() => expect(answered.length).toBe(13))

    await checkout.selectCurrency('usdc')
    const usdcReceipt = (await derivePaymentReceiptPda(usdcPda)).toString()
    // The switch checks the address it moved to straight away.
    await vi.waitFor(() => expect(chainAsked).toContain(usdcReceipt))
    const askedAtSwitch = chainAsked.length

    // The other currency is paid at a different address, so it is a new question:
    // it is asked again on the next tick, not the minute the running schedule was
    // already waiting.
    await vi.advanceTimersByTimeAsync(5_000)
    await vi.waitFor(() => expect(chainAsked.length).toBeGreaterThan(askedAtSwitch))
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
