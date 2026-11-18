import { address } from '@vue-solana/nuxt/kit'
import type { ComputedRef, Ref } from 'vue'
import { chainReadGapAfter } from '~/utils/chainReadGap'
import type { PaymentOption, PaymentQrPayload, PaymentRecord } from './usePaymentApi'
import { derivePaymentReceiptPda } from './usePaymentProgram'
import { STATUS_POLL_MS } from './checkoutVerification'

/** Reads the invoice's status from the API and the chain, on their two schedules. */
export function useCheckoutChain(deps: {
  payload: Ref<PaymentQrPayload | null>
  record: Ref<PaymentRecord | null>
  settled: ComputedRef<boolean>
  activePda: Ref<string | null>
  options: ComputedRef<PaymentOption[]>
  api: ReturnType<typeof usePaymentApi>
  client: ReturnType<typeof useSolanaClient>['client']
}) {
  const { payload, record, settled, activePda, options, api, client } = deps

  /**
   * Every address this code could have been paid on: the one being priced, which
   * follows the discount, and each offered currency's own. Nothing but the chain
   * knows which of them took the money.
   */
  function watchAddresses(): (string | null)[] {
    return [activePda.value, ...options.value.map(option => option.payment_pda)]
  }

  /**
   * Records a payment the chain already holds as settled, for the window the API
   * cannot close: a reload that lands in the slot or two between the cluster
   * confirming a send and the API's node voting it `confirmed`. Reading at
   * `confirmed` narrowed that window from a dozen seconds to about one; this read
   * closes the rest. It runs at load, on a currency switch, and then beside the
   * poll on the slower schedule `chainReadGapAfter` keeps — the poll's own answer
   * still comes from the API, every tick.
   *
   * Every `settle_*` instruction closes its invoice in the same transaction, so
   * the receipt is the only account a settled invoice leaves behind. One read per
   * address, in parallel: a code that takes SOL and USDC has an address per
   * currency and the chain is the only place that knows which one was paid. A
   * read that fails changes nothing — a flaky node must not talk the page out of
   * a payment the buyer can still make.
   */
  async function settleFromChain(pdas: (string | null)[]): Promise<void> {
    if (!record.value || settled.value) return
    const paid = (pda: string) =>
      derivePaymentReceiptPda(address(pda))
        .then(receipt =>
          client.rpc
            .getAccountInfo(receipt, { commitment: 'confirmed', encoding: 'base64' })
            .send()
        )
        .then(({ value }) => Boolean(value))
        .catch(() => false)

    const unique = [...new Set(pdas.filter((pda): pda is string => Boolean(pda)))]
    const settledSomewhere = await Promise.all(unique.map(paid))
    if (!settledSomewhere.some(Boolean)) return
    record.value = { ...record.value, status: 'settled' }
  }

  async function refreshRecord() {
    const current = payload.value
    const pda = activePda.value
    // A settled record that names its payer is the end of the story. One that
    // does not is worth another read: the chain settles the sheet before the API
    // does, so on a reload the payer is still on its way. Before that, the record
    // is worth reading again even after this page sent its own payment — that send
    // is what the receipt is waiting to show, and the page cannot tell settled
    // from still-settling without it.
    if (settled.value && record.value?.settled_by) {
      stopPolling()
      return
    }
    if (!current || !pda) return
    try {
      const fetched = await api.getPayment(pda, {
        cluster: current.cluster,
        endpoint: current.endpoint
      })
      if (settled.value) {
        // The chain got there first, so only the payer is missing — and only the
        // payer is taken, because a lagging answer must not unsettle an invoice
        // the chain already holds. Once the API agrees it is settled, there is
        // nothing left to ask.
        if (record.value && fetched.settled_by) {
          record.value = { ...record.value, settled_by: fetched.settled_by }
        }
        if (fetched.status === 'settled') stopPolling()
        return
      }
      // The API reads the cluster at `confirmed`, the same level the send waited
      // for, so this record catches up within a slot or two of a payment — a tick
      // or two behind it at most. The page needs no read of its own here.
      record.value = fetched
    } catch {
      // Keep the last known status; the next tick retries.
    }
  }

  /**
   * The page's own reads of the chain, beside the poll's API read: how many it has
   * made since the schedule started, when the last one went out, and whether one
   * is owed whatever the gap says. Only the chain read is spaced by these — the
   * API is asked on every tick as before, so the record still settles from the
   * API alone.
   */
  let chainReads = 0
  let lastChainReadAt = 0
  let chainReadPending = true

  /**
   * A chain read when the schedule says one is due: the wait after the last read
   * grows with every read, so an invoice nobody has paid costs the buyer's RPC a
   * read a minute rather than one every tick. A pending read owes one now, which
   * is how a restarted schedule reaches the tick after it. There is no settled
   * check here — `settleFromChain` refuses a settled record, so a paid page costs
   * nothing whichever tick the schedule reaches next.
   */
  async function readChainIfDue(): Promise<void> {
    if (!chainReadPending && Date.now() - lastChainReadAt < chainReadGapAfter(chainReads)) {
      return
    }
    chainReadPending = false
    lastChainReadAt = Date.now()
    chainReads += 1
    await settleFromChain(watchAddresses())
  }

  /**
   * Starts the schedule over: no reads yet, and the first one owed, so the next
   * poll tick reads however far into a tick's worth of time this call lands. A
   * currency switch arrives between ticks, and a wait measured from the switch
   * would miss the tick after it by a few milliseconds.
   */
  function restartChainReads() {
    chainReads = 0
    chainReadPending = true
  }

  let poll: ReturnType<typeof setInterval> | undefined

  function stopPolling() {
    if (poll) clearInterval(poll)
    poll = undefined
  }

  /** Starts the record poll that both reads ride on. */
  function startPolling() {
    poll = setInterval(() => {
      void refreshRecord()
      void readChainIfDue()
    }, STATUS_POLL_MS)
  }

  return {
    stopPolling,
    startPolling,
    watchAddresses,
    settleFromChain,
    restartChainReads
  }
}
