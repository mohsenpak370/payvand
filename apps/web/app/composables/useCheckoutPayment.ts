import { address, type Address } from '@vue-solana/nuxt/kit'
import type { ComputedRef, Ref } from 'vue'
import type { InvoiceCosts } from '~/utils/invoiceCost'
import {
  deriveAssociatedTokenAddress,
  deriveConfigPda,
  derivePaymentReceiptPda,
  deriveTreasuryPda,
  deriveTreasuryTokenAccount
} from './usePaymentProgram'
import type { PaymentOption, PaymentQrPayload, PaymentRecord } from './usePaymentApi'
import { HOLD_TICK_MS, HOLD_TO_PAY_MS } from './checkoutVerification'

/** The two ways of paying: the held allowance press and the wallet's approval. */
export function useCheckoutPayment(deps: {
  api: ReturnType<typeof usePaymentApi>
  budget: ReturnType<typeof useBudget>
  client: ReturnType<typeof useSolanaClient>['client']
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
    payableAmount, costs, costsError, refreshCosts, addressFor
  } = deps

  /**
   * The relayer to hand this settlement to, or null when the wallet has to pay
   * for it itself. The endpoint answers with its key and whether it will take
   * this payment; a request that fails is not a failed payment, so it answers
   * null and leaves the wallet's own path in place.
   */
  async function relayerAddress(): Promise<Address | null> {
    try {
      const info = await api.getSponsor()
      return info.available && info.address ? address(info.address) : null
    } catch {
      return null
    }
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
          const instruction = buildInstruction(
            'settle_payment',
            { amount, memo },
            {
              payer: payerAddress,
              payment,
              seller,
              receipt: await derivePaymentReceiptPda(payment),
              reference,
              config: await deriveConfigPda(),
              treasury: await deriveTreasuryPda()
            }
          )
          const wire = await compileWireTransaction({ instruction, payer: payerAddress, rpc: client.rpc })
          sent = await execute(wire, {
            confirm: true,
            confirmation: { commitment: 'confirmed', timeoutMs: 120_000 }
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
          // The fee leaves the buyer's account for the treasury's own token
          // account and the pool refunds the record rents, so the price is all
          // the buyer spends. The seller's account is theirs to have opened.
          //
          // Settling fronts two things before the same transaction refunds
          // them: the network fee and the two record rents. A wallet holding
          // them pays them itself and is its own sponsor, exactly as before.
          // Below that it cannot sign a settle at all, so the relayer's key
          // pays and the buyer only signs. Nothing is asked of the relayer
          // while the wallet can pay.
          const advances = quote.invoiceRent + quote.receiptRent + quote.networkFee
          const held = Number(
            (await client.rpc.getBalance(payerAddress, { commitment: 'confirmed' }).send()).value
          )
          const relayer = held < advances ? await relayerAddress() : null
          const feePayer = relayer ?? payerAddress

          const instruction = buildInstruction(
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
              // rents, and gets the pool's refund of them.
              sponsor: feePayer
            }
          )

          if (relayer) {
            // The buyer signs the transaction the relayer will submit, and the
            // relayer's own slot is left empty: the endpoint rebuilds this
            // message, compares it byte for byte, and only then signs its half.
            const transaction = await compileTransactionForSigning({
              instruction,
              payer: feePayer,
              rpc: client.rpc
            })
            // The fee payer's slot is empty on purpose, and the wallet hands
            // back these same bytes with the buyer's signature on them.
            const [signed] = await signTransactions([signableWireTransaction(transaction, feePayer)])
            sent = (await api.sponsorSettle({
              transaction: base64WireTransaction(signed!),
              buyer: payerAddress,
              seller,
              reference,
              mint,
              amount,
              memo,
              cluster: current.cluster,
              endpoint: current.endpoint
            })).signature
          } else {
            const wire = await compileWireTransaction({ instruction, payer: feePayer, rpc: client.rpc })
            sent = await execute(wire, {
              confirm: true,
              confirmation: { commitment: 'confirmed', timeoutMs: 120_000 }
            })
          }
        }
      }
      signature.value = sent

      record.value = await api.getPayment(payment, {
        cluster: current.cluster,
        endpoint: current.endpoint
      })
    } catch (cause) {
      failure.value = cause instanceof Error ? cause.message : appT('errors.notSent')
    } finally {
      busy.value = false
    }
  }

  return { holding, holdProgress, beginHold, stopHold, pay }
}
