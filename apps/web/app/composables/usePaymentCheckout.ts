import { address, type Address } from '@vue-solana/nuxt/kit'
import type {
  DiscountPreview,
  PaymentItem,
  PaymentOption,
  PaymentQrPayload,
  PaymentRecord,
  SupportedCurrency
} from './usePaymentApi'
import { discountedLamports, paymentOptions } from './usePaymentApi'
import {
  deriveAssociatedTokenAddress,
  deriveConfigPda,
  derivePaymentPda,
  derivePaymentReceiptPda,
  deriveTokenPaymentPda,
  deriveTreasuryPda,
  deriveTreasuryTokenAccount,
  mintForCurrency
} from './usePaymentProgram'
import { peekSessionSigner } from './useSessionKey'
import type { BudgetState, TokenBudgetState } from './useBudget'
import type { InvoiceCosts } from '~/utils/invoiceCost'

/**
 * Paying from the allowance needs no wallet approval, so the buyer's consent is
 * the press itself: the button has to be held for this long, or nothing is
 * sent. Nothing pays on its own, so a reload, a back navigation or a link
 * preview can never charge anyone.
 */
const HOLD_TO_PAY_MS = 3_000
/** How often the hold's fill and elapsed time are recomputed. */
const HOLD_TICK_MS = 50
const STATUS_POLL_MS = 5_000

/**
 * What the page can offer right now. The order is the priority: a paid invoice
 * is a receipt, and the allowance is always preferred over a wallet approval —
 * but only the buyer's press ever starts a payment.
 *
 * `pick`   no wallet in this browser yet → offer the wallet list
 * `wallet` a connected wallet approves it (`budgetState === null` there means
 *          no allowance exists yet, so the page offers to set one up)
 * `setup`  a budget exists, but this device holds no session key for it
 * `silent` the allowance covers it: one deliberate, held press pays it
 */
export type CheckoutStage
  = | 'loading'
    | 'invalid'
    | 'settled'
    | 'confirming'
    | 'silent'
    | 'wallet'
    | 'setup'
    | 'pick'

/**
 * Proves a code names what it charges: the program binds the seller, the
 * reference, the currency and the price into each invoice address, so details
 * edited apart cannot derive the address the code carries. Returns the options
 * it checked — every way that code can be paid.
 *
 * Shared with the scanner, which checks a code before showing any payment UI:
 * a code read off a printout has to fail at the camera, not one screen later.
 */
export async function verifyPaymentCode(parsed: PaymentQrPayload): Promise<PaymentOption[]> {
  const offered = paymentOptions(parsed)
  if (!offered.length) throw new Error(appT('errors.noPrice'))

  for (const option of offered) {
    const derived = option.currency === 'sol'
      ? await derivePaymentPda(
          address(parsed.seller),
          address(parsed.reference),
          option.amount
        )
      : await deriveTokenPaymentPda(
          address(parsed.seller),
          address(parsed.reference),
          address(option.mint),
          option.amount
        )
    if (derived !== option.payment_pda) {
      throw new Error(appT('errors.detailsMismatch'))
    }
  }

  return offered
}

/**
 * Resolves an invoice from the shared link and carries it to payment.
 *
 * The three ways to pay are deliberately separate states rather than one
 * button with a fallback chain, because each one needs the buyer to do
 * something different — nothing, approve, or open a different app.
 */
export function usePaymentCheckout() {
  const api = usePaymentApi()
  const { client } = useSolanaClient()
  const { publicKey, connected, canSignTransaction } = useSolanaWallet()
  const { execute } = useSolanaSignAndSendTransaction()
  const budget = useBudget()

  const payload = ref<PaymentQrPayload | null>(null)
  /** The seller's own code, verbatim, so the QR can re-encode exactly it. */
  const code = ref('')
  const record = ref<PaymentRecord | null>(null)
  const loading = ref(true)
  const error = ref('')
  const failure = ref('')
  const busy = ref(false)
  const signature = ref<string | null>(null)
  /**
   * The chain holds this invoice's payment, whatever the record says. A reload
   * lands in the seconds between a confirmed send and the commitment the API's
   * node reads, when the record still answers `pending` — the window in which
   * the button used to come back and the second press earned a chain error.
   */
  const onChainSettled = ref(false)

  const discount = ref<DiscountPreview | null>(null)
  const discountInput = ref('')
  const discountBusy = ref(false)
  const discountError = ref('')
  const activePda = ref<string | null>(null)

  // The seller decides which currencies an invoice takes; the buyer only
  // picks among them. `selected` is the one being priced and paid.
  const selectedCurrency = ref<SupportedCurrency>('sol')
  const options = computed<PaymentOption[]>(() =>
    payload.value ? paymentOptions(payload.value) : []
  )
  const selected = computed<PaymentOption>(() => {
    return (
      options.value.find(option => option.currency === selectedCurrency.value)
      ?? options.value[0]
      ?? { currency: 'sol', mint: '', decimals: 9, amount: 0, payment_pda: '' }
    )
  })
  const offersChoice = computed(() => options.value.length > 1)

  // The allowance path is three separate things agreeing: the connected wallet
  // says *who* pays, the budget says *how much* is allowed, and the session key
  // stored on this device says *how* to sign without a popup. Missing any one
  // of them is a different screen, not a generic error.
  const budgetState = ref<BudgetState | null>(null)
  const tokenBudgetState = ref<TokenBudgetState | null>(null)
  const budgetObstacle = ref<string | null>(null)
  const sessionReady = ref(false)

  /** 0..1 while the allowance button is held, 0 when it is not. */
  const holdProgress = ref(0)
  const holding = ref(false)

  /** What the chosen currency is charged, after any discount, in base units. */
  const payableAmount = computed(() =>
    discount.value
      ? discountedLamports(selected.value.amount, discount.value.percent_off)
      : selected.value.amount
  )
  const priceAmount = computed(() => selected.value.amount)
  // Kept for the SOL wording elsewhere on the page: an invoice priced in SOL.
  const payableLamports = computed(() => (selected.value.currency === 'sol' ? payableAmount.value : 0))
  const priceLamports = computed(() => (selected.value.currency === 'sol' ? priceAmount.value : 0))

  // What this payment costs, from the on-chain policy and the cluster's rent.
  // `null` until the first read, and again if `Config` cannot be read: the
  // checkout shows no total rather than one that assumes a zero fee.
  const { estimateInvoiceCosts } = useInvoiceCosts()
  const costs = ref<InvoiceCosts | null>(null)
  const costsError = ref('')

  async function refreshCosts(): Promise<InvoiceCosts | null> {
    const option = selected.value
    try {
      const quote = await estimateInvoiceCosts(payableAmount.value, option.currency, {
        mint: option.mint || undefined,
        seller: payload.value?.seller ? address(payload.value.seller) : undefined
      })
      costs.value = quote
      costsError.value = ''
      return quote
    } catch (cause) {
      costs.value = null
      costsError.value = cause instanceof Error ? cause.message : appT('errors.feeConfigUnavailable')
      return null
    }
  }
  const settled = computed(
    () => record.value?.status === 'settled' || onChainSettled.value
  )

  // Name and description ride in the code itself: the chain stores only the
  // memo, so a record read back by id comes without them.
  const paymentName = computed(() => record.value?.name || payload.value?.name || '')
  const paymentDescription = computed(
    () => record.value?.description || payload.value?.description || ''
  )
  const paymentMemo = computed(() => record.value?.memo || payload.value?.memo || '')
  // The receipt, if the seller built this invoice from their shop. It lives in
  // the code, not the account, so a chain read never loses it.
  const items = computed<PaymentItem[]>(() => payload.value?.items ?? [])

  /**
   * The fee's destination for this mint does not exist yet, so the program
   * would reject the settle after the buyer signed. That account is the
   * protocol's own setup — one call of `open_treasury_token_account` by the
   * deployer — never something a buyer can pay or fix, so the page says what is
   * missing and holds the button.
   */
  const treasuryTokenAccountMissing = computed(
    () => costs.value?.treasuryTokenAccountMissing === true
  )

  /**
   * The invoice itself, whatever signs: nothing is sent while one payment is in
   * flight, after one landed, or to a seller whose fee account does not exist.
   * An unpaid invoice is a record that is not settled — `pending` and the older
   * `missing` both mean nobody has paid it yet.
   *
   * `signature` is this page's own send: the chain has it, but the record the
   * page reads comes from the API and can still say `pending`. Pressing again in
   * that window only earns `AlreadySettled` from the program, so the button
   * stays away until the record catches up.
   */
  const invoicePayable = computed(
    () =>
      Boolean(record.value)
      && !settled.value
      && !busy.value
      && !signature.value
      && !treasuryTokenAccountMissing.value
  )

  /** The allowance that would pay this invoice, in whichever currency it is. */
  const activeBudget = computed(() =>
    selected.value.currency === 'sol' ? budgetState.value : tokenBudgetState.value
  )

  /** True when this browser can settle the invoice with no wallet popup. */
  const canPaySilently = computed(
    () => Boolean(activeBudget.value)
      && sessionReady.value
      && !budgetObstacle.value
      && !treasuryTokenAccountMissing.value
  )

  /**
   * The session key signs an allowance payment, so a wallet that cannot sign is
   * no obstacle to it — the wallet only says who is paying. Gating this path on
   * the wallet's signer is what made the hold button unavailable on wallets that
   * report they cannot sign.
   */
  const canPayFromAllowance = computed(
    () => connected.value && Boolean(publicKey.value) && invoicePayable.value && canPaySilently.value
  )

  /** A wallet payment is the wallet's own approval, so that wallet must sign. */
  const canPayWithWallet = computed(
    () => connected.value
      && Boolean(publicKey.value)
      && canSignTransaction.value
      && invoicePayable.value
  )

  /** Something can be paid right now, by either path. */
  const canPay = computed(() => canPayFromAllowance.value || canPayWithWallet.value)

  /**
   * What is in the way when no path can pay, in the buyer's words. The stage
   * picks the button; this explains a button that cannot be used. It stays empty
   * for what the page already says elsewhere: no wallet to connect yet, an
   * invoice that is paid, an allowance that cannot be used here (the stage's own
   * setup step or its obstacle), and a missing treasury account.
   */
  const blockedReason = computed(() => {
    if (!connected.value || !publicKey.value || settled.value) return ''
    if (canPaySilently.value || canSignTransaction.value) return ''
    return appT('checkout.walletCannotSign')
  })

  const stage = computed<CheckoutStage>(() => {
    if (loading.value) return 'loading'
    if (error.value || !payload.value) return 'invalid'
    if (settled.value) return 'settled'
    // Sent from here, not yet settled: the button stays out of reach while the
    // record catches up, and the poll below is what brings it in.
    if (signature.value) return 'confirming'
    if (!connected.value || !publicKey.value) return 'pick'
    if (canPaySilently.value) return 'silent'
    // A budget this device cannot sign for is a one-time setup step, not a
    // reason to fall back to a popup on every payment.
    if (activeBudget.value && !sessionReady.value) return 'setup'
    return 'wallet'
  })

  async function refreshBudget() {
    budgetState.value = null
    tokenBudgetState.value = null
    budgetObstacle.value = null
    sessionReady.value = false
    const payer = publicKey.value
    if (!payer) return

    const option = selected.value
    try {
      const mint = option.currency === 'sol'
        ? null
        : mintForCurrency(option.currency as 'usdc' | 'eurc', payload.value?.cluster ?? 'devnet')
      if (option.currency !== 'sol' && !mint) return

      if (mint) {
        const state = await budget.loadTokenBudget(payer, mint)
        tokenBudgetState.value = state
        if (!state) return
        const session = await peekSessionSigner(state.buyer)
        sessionReady.value = Boolean(session && session.address === state.sessionKey)
        budgetObstacle.value = sessionReady.value
          ? await budget.tokenBudgetObstacle(state, payableAmount.value)
          : null
        return
      }

      const state = await budget.loadBudget(payer)
      budgetState.value = state
      if (!state) return
      const session = await peekSessionSigner(state.buyer)
      sessionReady.value = Boolean(session && session.address === state.sessionKey)
      // Only ask the budget whether it covers this amount once we know it can
      // sign at all — otherwise every amount reports the same missing key.
      budgetObstacle.value = sessionReady.value
        ? await budget.silentPaymentObstacle(state, payableAmount.value)
        : null
    } catch {
      // A flaky RPC read must never block paying — the wallet path still works.
    }
  }

  /** The address the chosen currency, at the price shown, is watched on. */
  async function addressFor(option: PaymentOption, amount: number): Promise<Address> {
    const seller = address(payload.value?.seller ?? '')
    const reference = address(payload.value?.reference ?? '')
    if (option.currency === 'sol') {
      return await derivePaymentPda(seller, reference, amount)
    }
    return await deriveTokenPaymentPda(seller, reference, address(option.mint), amount)
  }

  /**
   * Whether the chain already holds this payment. `settle_*` creates the payment
   * account and its receipt in one instruction and nothing closes them, so
   * either one existing at `confirmed` means the invoice is paid — even while the
   * record still says `pending`.
   *
   * A read that fails answers false: a flaky node must not talk the page out of
   * a payment the buyer can still make.
   */
  async function paidOnChain(pda: string): Promise<boolean> {
    const payment = address(pda)
    const holds = async (key: Address) => {
      const info = await client.rpc
        .getAccountInfo(key, { commitment: 'confirmed', encoding: 'base64' })
        .send()
      return Boolean(info.value)
    }
    try {
      if (await holds(payment)) return true
      return await holds(await derivePaymentReceiptPda(payment))
    } catch {
      return false
    }
  }

  /**
   * Whether any address this code can be paid on holds a payment. A code that
   * takes SOL and USDC has one address per currency, and the chain is the only
   * place that knows which of them was already paid.
   */
  async function paidAnywhere(pdas: (string | null)[]): Promise<boolean> {
    const unique = [...new Set(pdas.filter((pda): pda is string => Boolean(pda)))]
    const found = await Promise.all(unique.map(paidOnChain))
    return found.some(Boolean)
  }

  /**
   * Switching currency moves the invoice to a different on-chain address, so
   * the watch address and the allowance follow it, and a held press is dropped.
   */
  async function selectCurrency(currency: SupportedCurrency) {
    if (currency === selectedCurrency.value) return
    stopHold()
    selectedCurrency.value = currency
    const option = selected.value
    if (option) {
      activePda.value = (await addressFor(option, payableAmount.value)).toString()
    }
    await Promise.all([refreshCosts(), refreshBudget()])
    // The invoice may have been paid on the currency being switched to, and one
    // already found stays found: switching cannot re-offer a paid invoice.
    onChainSettled.value
      = onChainSettled.value || await paidAnywhere([activePda.value])
  }

  async function refreshRecord() {
    const current = payload.value
    const pda = activePda.value
    // The record saying settled is the end of the story — not the chain read
    // that already took the button away: the receipt is where `settled_by`
    // comes from, so the poll keeps reading until it arrives.
    if (record.value?.status === 'settled') {
      stopPolling()
      return
    }
    if (!current || !pda) return
    try {
      record.value = await api.getPayment(pda, {
        cluster: current.cluster,
        endpoint: current.endpoint
      })
    } catch {
      // Keep the last known status; the next tick retries.
    }
  }

  let poll: ReturnType<typeof setInterval> | undefined
  let holdTimer: ReturnType<typeof setInterval> | undefined

  function stopPolling() {
    if (poll) clearInterval(poll)
    poll = undefined
  }

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
      if (!canPay.value || !canPaySilently.value) {
        stopHold()
        return
      }
      const elapsed = Date.now() - startedAt
      holdProgress.value = Math.min(1, elapsed / HOLD_TO_PAY_MS)
      if (elapsed >= HOLD_TO_PAY_MS) void pay(true)
    }, HOLD_TICK_MS)
  }

  async function load(source: string) {
    loading.value = true
    error.value = ''
    failure.value = ''
    signature.value = null
    record.value = null
    onChainSettled.value = false
    payload.value = null
    activePda.value = null
    discount.value = null
    discountInput.value = ''
    discountError.value = ''
    stopHold()
    stopPolling()

    try {
      const parsed = parseQrPayload(source)

      // The QR must re-encode the invoice, not the link it arrived on.
      code.value = unwrapQrText(source)
      payload.value = parsed

      const offered = await verifyPaymentCode(parsed)

      // SOL leads when the seller takes it, so the first price a buyer sees is
      // the one most of them can pay.
      selectedCurrency.value = offered.some(option => option.currency === 'sol')
        ? 'sol'
        : offered[0]!.currency
      activePda.value = selected.value.payment_pda
      record.value = await api.getPayment(activePda.value, {
        cluster: parsed.cluster,
        endpoint: parsed.endpoint
      })

      // A code the invoice carries applies itself, so the first price the
      // buyer sees is the one they pay.
      if (parsed.discount) await applyDiscount(parsed.discount.code)
      await Promise.all([refreshCosts(), refreshBudget()])

      // The record is what the API's node answers, and that read trails a
      // confirmed payment by a few seconds — exactly the moment a reload lands
      // in. The chain decides: a payment account or receipt on any address this
      // code can be paid on means the invoice is paid, so the button never comes
      // back for it.
      onChainSettled.value = await paidAnywhere([
        activePda.value,
        ...offered.map(option => option.payment_pda)
      ])

      poll = setInterval(() => {
        void refreshRecord()
      }, STATUS_POLL_MS)
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : appT('errors.unreadableLink')
    } finally {
      loading.value = false
    }
  }

  /**
   * Sends the invoice. A wallet payment is the buyer's tap, but a payment from
   * the monthly allowance is signed by this device's session key with no prompt
   * at all, so it runs only when the button's hold completed: `fromHold` is
   * that confirmation, and nothing else may claim it.
   */
  async function pay(fromHold?: true) {
    const current = payload.value
    const payer = publicKey.value
    // A payment attempt always ends the press. If this one turns out to be a
    // no-op, the interval must not call it again on every tick.
    stopHold()
    // A send of this page's own, or a payment the chain already holds, is
    // enough: a second attempt is rejected on chain, so nothing is built for it.
    if (!current || !payer || busy.value || signature.value || settled.value) return
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
          sent = await budget.sendSilentPayment(payer, {
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
              payer,
              payment,
              seller,
              receipt: await derivePaymentReceiptPda(payment),
              reference,
              config: await deriveConfigPda(),
              treasury: await deriveTreasuryPda()
            }
          )
          const wire = await compileWireTransaction({ instruction, payer, rpc: client.rpc })
          sent = await execute(wire, {
            confirm: true,
            confirmation: { commitment: 'confirmed', timeoutMs: 120_000 }
          })
        }
      } else {
        const mint = address(option.mint)
        if (fromAllowance) {
          // A fresh seller's account and this payment's two records come out of
          // the floats the session key and the budget keep, so a third such
          // payment in a row runs out of lamports. One approval refills just the
          // gap and the payment follows it, still without a second prompt.
          const refill = await budget.tokenFloatTopUpInstructions(payer, mint)
          if (refill.length > 0) {
            const wire = await compileWireTransaction({ instruction: refill, payer, rpc: client.rpc })
            await execute(wire, {
              confirm: true,
              confirmation: { commitment: 'confirmed', timeoutMs: 120_000 }
            })
          }
          sent = await budget.sendSilentTokenPayment(payer, {
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
          const instruction = buildInstruction(
            'settle_token_payment',
            { amount, memo },
            {
              payer,
              payment,
              seller,
              seller_token_account: await deriveAssociatedTokenAddress(seller, mint),
              payer_token_account: await deriveAssociatedTokenAddress(payer, mint),
              mint,
              receipt: await derivePaymentReceiptPda(payment),
              reference,
              config: await deriveConfigPda(),
              treasury: await deriveTreasuryPda(),
              treasury_token_account: await deriveTreasuryTokenAccount(mint)
            }
          )
          const wire = await compileWireTransaction({ instruction, payer, rpc: client.rpc })
          sent = await execute(wire, {
            confirm: true,
            confirmation: { commitment: 'confirmed', timeoutMs: 120_000 }
          })
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

  /** Ask the API to price the invoice with a code — typed, or carried by it. */
  async function applyDiscount(next = discountInput.value) {
    const current = payload.value
    if (!current?.seller_user) return

    // The price is about to move, so a held press must stop and let the buyer
    // see what they are now paying.
    stopHold()
    discountBusy.value = true
    discountError.value = ''
    try {
      const preview = await api.validateDiscount({
        code: next,
        seller: current.seller,
        // The API owns the percentage; the price it applies to is the one the
        // buyer is looking at, in whichever currency that is.
        price_lamports: priceAmount.value
      })
      discount.value = preview
      // The buyer is now looking at a different price, so the fee and rent
      // split moves with it.
      void refreshCosts()
      discountInput.value = ''
      // The discounted amount derives its own address — watch and pay that.
      activePda.value = (await addressFor(selected.value, payableAmount.value)).toString()
      await refreshBudget()
    } catch (cause) {
      discountError.value = apiErrorMessage(cause, appT('errors.discountFailed'))
    } finally {
      discountBusy.value = false
    }
  }

  function clearDiscount() {
    discount.value = null
    discountInput.value = ''
    discountError.value = ''
    activePda.value = selected.value.payment_pda || null
    void Promise.all([refreshCosts(), refreshBudget()])
  }

  /**
   * A wallet that arrives late — the extension finishing its injection, or the
   * buyer returning from a wallet app — decides which button the page can
   * offer, so the allowance is read again. It never sends anything by itself.
   */
  watch(publicKey, async () => {
    await refreshBudget()
  })

  onBeforeUnmount(() => {
    stopHold()
    stopPolling()
  })

  return {
    payload,
    code,
    options,
    selected,
    selectedCurrency,
    selectCurrency,
    offersChoice,
    payableAmount,
    priceAmount,
    activeBudget,
    tokenBudgetState,
    record,
    loading,
    error,
    failure,
    busy,
    signature,
    onChainSettled,
    discount,
    discountInput,
    discountBusy,
    discountError,
    budgetState,
    budgetObstacle,
    sessionReady,
    holding,
    holdProgress,
    beginHold,
    stopHold,
    payableLamports,
    priceLamports,
    costs,
    costsError,
    settled,
    paymentName,
    paymentDescription,
    paymentMemo,
    items,
    canPay,
    canPayFromAllowance,
    canPayWithWallet,
    blockedReason,
    treasuryTokenAccountMissing,
    stage,
    load,
    pay,
    applyDiscount,
    clearDiscount
  }
}
