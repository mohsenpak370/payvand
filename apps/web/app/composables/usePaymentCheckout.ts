import { address, type Address } from '@vue-solana/nuxt/kit'
import type {
  DiscountPreview,
  PaymentOption,
  PaymentQrPayload,
  PaymentRecord,
  SupportedCurrency
} from './usePaymentApi'
import { discountedLamports, paymentOptions } from './usePaymentApi'
import {
  deriveAssociatedTokenAddress,
  derivePaymentPda,
  deriveTokenPaymentPda,
  mintForCurrency
} from './usePaymentProgram'
import { peekSessionSigner } from './useSessionKey'
import type { BudgetState, TokenBudgetState } from './useBudget'

/**
 * Paying from the allowance is the only path that needs no wallet at all, so
 * it carries the flow — but it still waits long enough to be stopped, and it
 * fires at most once per invoice per browser. A reload, a back navigation or a
 * link preview can then never charge someone twice.
 */
const AUTO_PAY_SECONDS = 3
const STATUS_POLL_MS = 5_000

const autoPayKey = (paymentId: string) => `payvand.autopay.${paymentId}`

/**
 * What the page can offer right now. The order is the priority: a paid invoice
 * is a receipt, a running countdown outranks every button, and the allowance is
 * always preferred over a wallet approval.
 *
 * `pick`   no wallet in this browser yet → offer the wallet list
 * `wallet` a connected wallet approves it (`budgetState === null` there means
 *          no allowance exists yet, so the page offers to set one up)
 * `setup`  a budget exists, but this device holds no session key for it
 */
export type CheckoutStage
  = | 'loading'
    | 'invalid'
    | 'settled'
    | 'auto'
    | 'silent'
    | 'wallet'
    | 'setup'
    | 'pick'

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
  /** True when the seller offered more than one way to pay. */
  const offersChoice = computed(() => options.value.length > 1)

  // The allowance path is three separate things agreeing: the connected wallet
  // says *who* pays, the budget says *how much* is allowed, and the session key
  // stored on this device says *how* to sign without a popup. Missing any one
  // of them is a different screen, not a generic error.
  const budgetState = ref<BudgetState | null>(null)
  const tokenBudgetState = ref<TokenBudgetState | null>(null)
  const budgetObstacle = ref<string | null>(null)
  const sessionReady = ref(false)

  const autoSeconds = ref(0)
  const autoAttempted = ref(false)

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
  const settled = computed(() => record.value?.status === 'settled')

  // Name and description ride in the code itself: the chain stores only the
  // memo, so a record read back by id comes without them.
  const paymentName = computed(() => record.value?.name || payload.value?.name || '')
  const paymentDescription = computed(
    () => record.value?.description || payload.value?.description || ''
  )
  const paymentMemo = computed(() => record.value?.memo || payload.value?.memo || '')

  /**
   * `missing` is the normal state of a fresh invoice: its account is created
   * inside the buyer's own transaction, so nothing watches it beforehand.
   */
  const canPay = computed(
    () =>
      connected.value
      && canSignTransaction.value
      && !busy.value
      && !settled.value
      && (record.value?.status === 'pending' || record.value?.status === 'missing')
  )

  /** The allowance that would pay this invoice, in whichever currency it is. */
  const activeBudget = computed(() =>
    selected.value.currency === 'sol' ? budgetState.value : tokenBudgetState.value
  )

  /** True when this browser can settle the invoice with no wallet popup. */
  const canPaySilently = computed(
    () => Boolean(activeBudget.value) && sessionReady.value && !budgetObstacle.value
  )

  const stage = computed<CheckoutStage>(() => {
    if (loading.value) return 'loading'
    if (error.value || !payload.value) return 'invalid'
    if (settled.value) return 'settled'
    if (autoSeconds.value > 0) return 'auto'
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
   * Switching currency moves the invoice to a different on-chain address, so
   * the watch address, the allowance and the countdown all follow it.
   */
  async function selectCurrency(currency: SupportedCurrency) {
    if (currency === selectedCurrency.value) return
    cancelAutoPay()
    selectedCurrency.value = currency
    const option = selected.value
    if (option) {
      activePda.value = (await addressFor(option, payableAmount.value)).toString()
    }
    await refreshBudget()
  }

  async function refreshRecord() {
    const current = payload.value
    const pda = activePda.value
    if (!current || !pda || signature.value) return
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
  let countdown: ReturnType<typeof setInterval> | undefined

  function stopPolling() {
    if (poll) clearInterval(poll)
    poll = undefined
  }

  function cancelAutoPay() {
    if (countdown) clearInterval(countdown)
    countdown = undefined
    autoSeconds.value = 0
  }

  function startCountdown() {
    cancelAutoPay()
    autoSeconds.value = AUTO_PAY_SECONDS
    countdown = setInterval(() => {
      autoSeconds.value -= 1
      if (autoSeconds.value <= 0) {
        cancelAutoPay()
        void pay()
      }
    }, 1_000)
  }

  /**
   * The automatic payment: only for an invoice this browser has not paid
   * before, only when the budget can cover it, and only once the page has
   * everything it needs to send it.
   */
  function maybeAutoPay() {
    const current = payload.value
    if (!current || !record.value) return
    if (record.value.status === 'settled' || autoAttempted.value) return
    if (!canPay.value || !canPaySilently.value) return

    autoAttempted.value = true
    try {
      sessionStorage.setItem(autoPayKey(activePda.value ?? current.id), '1')
    } catch {
      // Private mode: the in-memory flag still stops a double send.
    }
    startCountdown()
  }

  async function load(source: string) {
    loading.value = true
    error.value = ''
    failure.value = ''
    signature.value = null
    record.value = null
    payload.value = null
    activePda.value = null
    discount.value = null
    discountInput.value = ''
    discountError.value = ''
    cancelAutoPay()
    stopPolling()

    try {
      const parsed = parseQrPayload(source)

      // The QR must re-encode the invoice, not the link it arrived on.
      code.value = unwrapQrText(source)
      payload.value = parsed

      // Every currency is checked against its own seeds: the program binds
      // the token and the price into the address, so details edited apart
      // cannot derive the address the code names.
      const offered = paymentOptions(parsed)
      if (!offered.length) throw new Error('This payment code has no price.')
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
          throw new Error('These details do not match the code. Ask the seller for a new link.')
        }
      }

      // SOL leads when the seller takes it, so the first price a buyer sees is
      // the one most of them can pay.
      selectedCurrency.value = offered.some(option => option.currency === 'sol')
        ? 'sol'
        : offered[0]!.currency
      activePda.value = selected.value.payment_pda
      autoAttempted.value = sessionStorage.getItem(autoPayKey(parsed.id)) === '1'
      record.value = await api.getPayment(activePda.value, {
        cluster: parsed.cluster,
        endpoint: parsed.endpoint
      })

      // A code the invoice carries applies itself, so the first price the
      // buyer sees is the one they pay.
      if (parsed.discount) await applyDiscount(parsed.discount.code)
      await refreshBudget()

      poll = setInterval(() => {
        void refreshRecord()
      }, STATUS_POLL_MS)
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : 'Could not read this payment link.'
    } finally {
      loading.value = false
    }

    maybeAutoPay()
  }

  async function pay() {
    const current = payload.value
    const payer = publicKey.value
    if (!current || !payer || busy.value) return

    cancelAutoPay()
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
      const payment = await addressFor(option, amount)

      let sent: string
      if (option.currency === 'sol') {
        if (canPaySilently.value) {
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
            { payer, payment, seller, reference }
          )
          const wire = await compileWireTransaction({ instruction, payer, rpc: client.rpc })
          sent = await execute(wire, {
            confirm: true,
            confirmation: { commitment: 'confirmed', timeoutMs: 120_000 }
          })
        }
      } else {
        const mint = address(option.mint)
        if (canPaySilently.value) {
          sent = await budget.sendSilentTokenPayment(payer, {
            payment,
            seller,
            reference,
            mint,
            amount,
            memo
          })
        } else {
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
              reference
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
      failure.value = cause instanceof Error ? cause.message : 'The payment could not be sent.'
    } finally {
      busy.value = false
    }
  }

  /** Ask the API to price the invoice with a code — typed, or carried by it. */
  async function applyDiscount(next = discountInput.value) {
    const current = payload.value
    if (!current?.seller_user) return

    // The price is about to move, so an automatic payment must stop and let
    // the buyer see what they are now paying.
    cancelAutoPay()
    discountBusy.value = true
    discountError.value = ''
    try {
      const preview = await api.validateDiscount({
        code: next,
        seller_user: current.seller_user,
        // The API owns the percentage; the price it applies to is the one the
        // buyer is looking at, in whichever currency that is.
        price_lamports: priceAmount.value
      })
      discount.value = preview
      discountInput.value = ''
      // The discounted amount derives its own address — watch and pay that.
      activePda.value = (await addressFor(selected.value, payableAmount.value)).toString()
      await refreshBudget()
    } catch (cause) {
      discountError.value = apiErrorMessage(cause, 'That discount code could not be applied.')
    } finally {
      discountBusy.value = false
    }
  }

  function clearDiscount() {
    discount.value = null
    discountInput.value = ''
    discountError.value = ''
    activePda.value = selected.value.payment_pda || null
    void refreshBudget()
  }

  /**
   * A wallet that arrives late — the extension finishing its injection, or the
   * buyer returning from a wallet app — is exactly the moment the silent path
   * becomes possible, so the automatic payment is re-considered once.
   */
  watch(publicKey, async () => {
    await refreshBudget()
    maybeAutoPay()
  })

  onBeforeUnmount(() => {
    cancelAutoPay()
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
    discount,
    discountInput,
    discountBusy,
    discountError,
    budgetState,
    budgetObstacle,
    sessionReady,
    autoSeconds,
    payableLamports,
    priceLamports,
    settled,
    paymentName,
    paymentDescription,
    paymentMemo,
    canPay,
    stage,
    load,
    pay,
    applyDiscount,
    clearDiscount,
    cancelAutoPay
  }
}
