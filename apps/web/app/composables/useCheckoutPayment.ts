import { address, type Address, type Instruction } from '@vue-solana/nuxt/kit'
import type { ComputedRef, Ref } from 'vue'
import type { InvoiceCosts } from '~/utils/invoiceCost'
import {
  computeUnitLimitInstruction,
  deriveAssociatedTokenAddress,
  deriveConfigPda,
  derivePaymentReceiptPda,
  deriveTreasuryPda,
  deriveTreasuryTokenAccount
} from './usePaymentProgram'
import {
  lamportsToSol,
  type PaymentOption,
  type PaymentQrPayload,
  type PaymentRecord
} from './usePaymentApi'
import { HOLD_TICK_MS, HOLD_TO_PAY_MS } from './checkoutVerification'

/** The price as the buyer reads it: "1 SOL", "1.5 USDC". */
function priceLabel(option: PaymentOption, amount: number): string {
  const decimals = option.currency === 'sol' ? 9 : option.decimals
  // Only a fractional part loses its trailing zeros: a whole "10" stays 10.
  const value = option.currency === 'sol'
    ? lamportsToSol(amount)
    : (amount / 10 ** decimals).toFixed(decimals).replace(/\.(\d*?)0+$/, '.$1').replace(/\.$/, '')
  return `${value} ${option.currency.toUpperCase()}`
}

/**
 * The refusal a wallet gets when the price is more than it holds. Both paths
 * pay the price from the wallet — a relayer only covers the fee and the two
 * record rents — so this is the one thing no sponsorship can make up for, and
 * the chain would refuse the settlement after the buyer had signed it.
 */
function shortOnPrice(option: PaymentOption, amount: number): string {
  return appT('errors.walletShortOnPrice', { price: priceLabel(option, amount) })
}

/**
 * The buyer's balance in one token, or 0 when the account does not exist. Any
 * other failure is thrown: a cluster that did not answer is not a wallet that
 * cannot afford the price, and saying so would send the buyer to add funds
 * they already have.
 */
async function tokenBalance(
  client: ReturnType<typeof useSolanaClient>['client'],
  owner: Address,
  mint: Address
): Promise<number> {
  const account = await deriveAssociatedTokenAddress(owner, mint)
  const info = await client.rpc
    .getAccountInfo(account, { commitment: 'confirmed', encoding: 'base64' })
    .send()
  // No account is no balance to pay with, and the refusal names the price.
  if (!info.value) return 0
  const balance = await client.rpc
    .getTokenAccountBalance(account, { commitment: 'confirmed' })
    .send()
  return Number(balance.value.amount)
}

/**
 * What to show when a payment fails. The API explains a refused send in
 * `{ error }`, which `$fetch` buries in the thrown value's `data`: without this
 * the buyer reads "400 Bad Request" and nothing about what was objected to.
 */
function failureMessage(cause: unknown): string {
  const data = (cause as { data?: { error?: unknown } } | null)?.data
  if (data && typeof data.error === 'string' && data.error.trim()) return data.error
  return cause instanceof Error ? cause.message : appT('errors.notSent')
}

/** The two ways of paying: the held allowance press and the wallet's approval. */
export function useCheckoutPayment(deps: {
  api: ReturnType<typeof usePaymentApi>
  budget: ReturnType<typeof useBudget>
  client: ReturnType<typeof useSolanaClient>['client']
  /** Starts the page's own minute of receipt reads over after a send. */
  restartChainReads: () => void
  execute: ReturnType<typeof useSolanaSignAndSendTransaction>['execute']
  signTransactions: ReturnType<typeof useSolanaSignTransactions>['execute']
  payer: () => Address | null
  payload: Ref<PaymentQrPayload | null>
  record: Ref<PaymentRecord | null>
  busy: Ref<boolean>
  signature: Ref<string | null>
  failure: Ref<string>
  settled: ComputedRef<boolean>
  canPaySilently: ComputedRef<boolean>
  canPayFromAllowance: ComputedRef<boolean>
  selected: ComputedRef<PaymentOption>
  payableAmount: ComputedRef<number>
  costs: Ref<InvoiceCosts | null>
  costsError: Ref<string>
  refreshCosts: () => Promise<InvoiceCosts | null>
  addressFor: (option: PaymentOption, amount: number) => Promise<Address>
}) {
  const {
    api, budget, client, execute, signTransactions, payer, payload, record, busy,
    signature, failure, settled, canPaySilently, canPayFromAllowance, selected,
    payableAmount, costs, costsError, refreshCosts, addressFor, restartChainReads
  } = deps

  /**
   * The relayer to hand this settlement to, or null when the wallet has to pay
   * for it itself. The endpoint answers with its key and whether it will take
   * this payment — and this wallet's own daily allowance, so the answer is the
   * same for a buyer who never signed in. A request that fails is not a failed
   * payment, so it answers null and leaves the wallet's own path in place.
   */
  async function relayerAddress(wallet?: string): Promise<Address | null> {
    try {
      const info = await api.getSponsor(wallet)
      return info.available && info.address ? address(info.address) : null
    } catch {
      return null
    }
  }

  /**
   * Sends a wallet payment's settle with Payvand paying for it. This is the one
   * rule of every wallet payment, SOL or token (`docs/payment-costs.md`):
   *
   * 1. The relayer pays. The API builds the settle with the relayer as fee
   *    payer and sponsor and signs it first; this page checks it is exactly
   *    `settleFor(relayer)` before the wallet sees it; the wallet signs a
   *    transaction complete except for its own signature — so it can simulate
   *    it and preview the price — and the API sends it. The program refunds the
   *    relayer and charges the seller in that same transaction.
   * 2. Only when no relayer will pay (none configured, or a daily cap reached)
   *    does the wallet pay its own way, as its own sponsor. The program refunds
   *    it both rents and its signature in the same instruction, and the
   *    compute-unit limit keeps the wallet from adding a priority price nothing
   *    refunds, so the buyer still spends exactly the price. It only needs the
   *    lamports to front that for a moment; without them the refusal comes
   *    before a signature.
   *
   * The wallet is never the fee payer while a relayer is available: that would
   * have the buyer front what the platform pays.
   */
  async function settleThroughRelayer(input: {
    payerAddress: Address
    /** The wallet's lamports when already read (SOL path), or null. */
    held: number | null
    quote: InvoiceCosts
    /** Empty for a SOL invoice. */
    mint: string
    amount: number
    memo: string
    seller: Address
    reference: Address
    settleFor: (sponsor: Address) => Promise<Instruction>
  }): Promise<string> {
    const current = payload.value!
    const relayer = await relayerAddress(input.payerAddress)
    if (relayer) {
      const fields = {
        buyer: input.payerAddress,
        seller: input.seller,
        reference: input.reference,
        mint: input.mint,
        amount: input.amount,
        memo: input.memo,
        cluster: current.cluster,
        endpoint: current.endpoint
      }
      const instruction = await input.settleFor(relayer)
      const prepared = await api.sponsorPrepare(fields)
      const wire = wireFromBase64(prepared.transaction)
      // The bytes the buyer signs came from a server: they have to be this
      // invoice's settle and nothing else before any wallet prompt opens.
      if (!preparedSettleMatches(wire, relayer, instruction)) {
        throw new Error(appT('errors.preparedMismatch'))
      }
      const [signed] = await signTransactions([wire])
      return (await api.sponsorSettle({
        ...fields,
        transaction: base64WireTransaction(signed!)
      })).signature
    }

    // No relayer: the wallet fronts the rents and its one signature for the
    // length of the transaction. `sponsorshipFee` is quoted for the relayer's
    // two signatures, so it covers the wallet's one with room to spare.
    const advances = input.quote.invoiceRent + input.quote.receiptRent + input.quote.sponsorshipFee
    const extra = input.mint ? 0 : input.amount
    const held = input.held ?? Number(
      (await client.rpc.getBalance(input.payerAddress, { commitment: 'confirmed' }).send()).value
    )
    if (held < extra + advances) throw new Error(appT('errors.paymentNeedsSol'))
    const wire = await compileWireTransaction({
      instruction: [computeUnitLimitInstruction(), await input.settleFor(input.payerAddress)],
      payer: input.payerAddress,
      rpc: client.rpc
    })
    return await execute(wire, {
      confirm: true,
      confirmation: { commitment: 'confirmed', timeoutMs: 120_000 }
    })
  }

  /** 0..1 while the allowance button is held, 0 when it is not. */
  const holdProgress = ref(0)
  const holding = ref(false)
  let holdTimer: ReturnType<typeof setInterval> | undefined

  /**
   * Releases the allowance button: the hold is abandoned and nothing is sent.
   * Safe to call when no hold is running, which is what every cancel path does.
   */
  function stopHold() {
    if (holdTimer) clearInterval(holdTimer)
    holdTimer = undefined
    holding.value = false
    holdProgress.value = 0
  }

  /**
   * Starts the allowance's deliberate press. `stopHold` ends it — the button
   * does that when the pointer is released or the capture is lost, so a finger
   * that drifts a little is still the same press — and so does tabbing away or
   * the budget going out of reach. Only a press held for the whole count pays.
   */
  function beginHold() {
    // A press already under way keeps its own clock. A held space bar repeats
    // keydown, and a second finger is not a new press; either would restart the
    // count and the hold could never finish.
    if (holding.value || !canPayFromAllowance.value) return
    holding.value = true
    const startedAt = Date.now()
    holdTimer = setInterval(() => {
      // The press belongs to the allowance: if this browser can no longer pay
      // from it, or can no longer pay this invoice at all, the count is over.
      if (!canPayFromAllowance.value) {
        stopHold()
        return
      }
      const elapsed = Date.now() - startedAt
      holdProgress.value = Math.min(1, elapsed / HOLD_TO_PAY_MS)
      if (elapsed >= HOLD_TO_PAY_MS) void pay(true)
    }, HOLD_TICK_MS)
  }

  /**
   * Sends the invoice. A wallet payment is the buyer's tap, but a payment from
   * the monthly allowance is signed by this device's session key with no prompt
   * at all, so it runs only when the button's hold completed: `fromHold` is
   * that confirmation, and nothing else may claim it.
   */
  async function pay(fromHold?: true) {
    const current = payload.value
    const payerAddress = payer()
    // A payment attempt always ends the press. If this one turns out to be a
    // no-op, the interval must not call it again on every tick.
    stopHold()
    // A send of this page's own, or a payment the chain already holds, is
    // enough: a second attempt is rejected on chain, so nothing is built for it.
    if (!current || !payerAddress || busy.value || signature.value || settled.value) return
    // Read once: this browser's session key signs without a prompt, so only a
    // completed hold may ask for that. Reading it again after the awaits below
    // would let a budget that appears mid-flight turn a tap into a silent send.
    const fromAllowance = canPaySilently.value
    if (fromAllowance && !fromHold) return

    failure.value = ''
    busy.value = true
    try {
      const seller = address(current.seller)
      const reference = address(current.reference)
      // Full price unless a code was applied. `load()` already proved the code
      // matches its details; here the amount only derives the address that gets
      // paid, so what is paid is exactly what was shown.
      const option = selected.value
      const amount = payableAmount.value
      const memo = current.memo || ''

      // Preflight against the numbers the program will use: a policy the client
      // could not read is not a reason to guess, and both checks below are
      // rejected on chain — after the buyer had signed. The first also disables
      // the button, so this is what guards every other caller; the test beside
      // this file fails if either goes away.
      const quote = costs.value ?? await refreshCosts()
      if (!quote) throw new Error(costsError.value || appT('errors.feeConfigUnavailable'))
      if (quote.treasuryTokenAccountMissing) throw new Error(appT('errors.treasuryTokenAccountMissing'))
      if (amount < quote.minPayable) throw new Error(appT('errors.invoiceTooSmall'))

      const payment = await addressFor(option, amount)

      let sent: string
      if (option.currency === 'sol') {
        if (fromAllowance) {
          sent = await budget.sendSilentPayment(payerAddress, {
            payment,
            seller,
            reference,
            amount,
            memo
          })
        } else {
          // The relayer pays the fee and fronts the two records, never the
          // price: a wallet that does not hold the price cannot pay this
          // invoice at all, whatever it sends. Saying so before the wallet
          // opens is what keeps it from signing a transaction the chain refuses.
          const held = Number(
            (await client.rpc.getBalance(payerAddress, { commitment: 'confirmed' }).send()).value
          )
          if (held < amount) throw new Error(shortOnPrice(option, amount))

          sent = await settleThroughRelayer({
            payerAddress,
            held,
            quote,
            mint: '',
            amount,
            memo,
            seller,
            reference,
            settleFor: async sponsor => buildInstruction(
              'settle_payment',
              { amount, memo },
              {
                payer: payerAddress,
                payment,
                seller,
                receipt: await derivePaymentReceiptPda(payment),
                reference,
                config: await deriveConfigPda(),
                treasury: await deriveTreasuryPda(),
                sponsor
              }
            )
          })
        }
      } else {
        const mint = address(option.mint)
        if (fromAllowance) {
          // This payment's two records come out of the session float, which the
          // program refunds in the same transaction, so the key spends only its
          // signature. One approval refills whatever gap is left before the
          // advance, and the payment follows it without a second prompt.
          const refill = await budget.tokenFloatTopUpInstructions(payerAddress, mint)
          if (refill.length > 0) {
            const wire = await compileWireTransaction({ instruction: refill, payer: payerAddress, rpc: client.rpc })
            await execute(wire, {
              confirm: true,
              confirmation: { commitment: 'confirmed', timeoutMs: 120_000 }
            })
          }
          sent = await budget.sendSilentTokenPayment(payerAddress, {
            payment,
            seller,
            reference,
            mint,
            amount,
            memo
          })
        } else {
          // The price is in the token, so the wallet has to hold it whoever
          // pays the fee: an account with less than the price can only end in a
          // signature the chain refuses.
          const heldTokens = await tokenBalance(client, payerAddress, mint).catch(() => {
            throw new Error(appT('errors.balanceUnreadable'))
          })
          if (heldTokens < amount) throw new Error(shortOnPrice(option, amount))

          // The fee leaves the buyer's account for the treasury's own token
          // account and the treasury refunds the record rents in SOL, so the
          // price is all the buyer spends. The seller's account is theirs to
          // have opened.
          sent = await settleThroughRelayer({
            payerAddress,
            held: null,
            quote,
            mint,
            amount,
            memo,
            seller,
            reference,
            settleFor: async sponsor => buildInstruction(
              'settle_token_payment',
              { amount, memo },
              {
                payer: payerAddress,
                payment,
                seller,
                seller_token_account: await deriveAssociatedTokenAddress(seller, mint),
                payer_token_account: await deriveAssociatedTokenAddress(payerAddress, mint),
                mint,
                receipt: await derivePaymentReceiptPda(payment),
                reference,
                config: await deriveConfigPda(),
                treasury: await deriveTreasuryPda(),
                treasury_token_account: await deriveTreasuryTokenAccount(mint),
                // The key that pays the network fee and advances both record
                // rents, and gets the treasury's refund of them.
                sponsor
              }
            )
          })
        }
      }
      signature.value = sent
      // The send is the other moment the chain leads the API: a fresh minute of
      // the page's own receipt reads starts here, even on a page that has been
      // open longer than one.
      restartChainReads()

      record.value = await api.getPayment(payment, {
        cluster: current.cluster,
        endpoint: current.endpoint
      })
    } catch (cause) {
      failure.value = failureMessage(cause)
    } finally {
      busy.value = false
    }
  }

  return { holding, holdProgress, beginHold, stopHold, pay }
}
