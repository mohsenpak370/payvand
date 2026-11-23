import type { SupportedCurrency } from './usePaymentApi'
import type { SubscriptionQuote, SubscriptionStatus } from './useSubscription'
import { peekSessionSigner } from './useSessionKey'
import { coveredCurrency, orderedQuotes } from '~/utils/subscriptionQuotes'

/**
 * The shops subscription as a purchase: where the account stands, what the plan
 * costs in each currency, and the two steps that buy it.
 *
 * Shared state, because the shops page shows the same purchase twice — the card
 * at the top of the page and, when no subscription is active, the offer inside
 * the New shop dialog. One status, one invoice, one navigation, whichever of the
 * two the person uses.
 */
const status = shallowRef<SubscriptionStatus | null>(null)
const loading = ref(false)
const failure = ref('')
const paying = ref<SupportedCurrency | ''>('')
/**
 * The currencies this device can settle with no wallet popup: an allowance that
 * covers the quote *and* the session key that signs for it on this browser.
 * The checkout applies the same test before it offers its silent path.
 */
const silent = ref<Partial<Record<SupportedCurrency, boolean>>>({})
let inflight: Promise<void> | null = null

export function useSubscriptionPurchase() {
  const api = useSubscription()
  const budget = useBudget()
  const wallet = useSolanaWallet()
  const { t } = useI18n()
  const cluster = useRuntimeConfig().public.cluster as string

  /**
   * The first currency a silent payment can actually settle, which is the one
   * worth offering first. A balance alone is not enough: without this device's
   * budget session key the checkout still opens the wallet, so a currency it
   * cannot sign for is a plain price like the others.
   */
  const covered = computed<SupportedCurrency | null>(() =>
    coveredCurrency(status.value?.quotes ?? [], silent.value)
  )

  /** The plan's prices, with the one an allowance already covers first. */
  const quotes = computed<SubscriptionQuote[]>(() =>
    orderedQuotes(status.value?.quotes ?? [], covered.value)
  )

  const active = computed(() => Boolean(status.value?.active))

  /** One currency's price, in the money the person hands over. */
  function priceLabel(quote: SubscriptionQuote): string {
    return quote.currency === 'sol'
      ? `${lamportsToSol(quote.amount)} SOL`
      : `${formatTokenAmount(quote.amount, 6)} ${currencySymbol(quote.currency)}`
  }

  /**
   * Works out, per currency, whether this device can pay the quote without a
   * wallet: the allowance must exist, this browser must hold the session key
   * that allowance names, and the allowance must say it covers this amount —
   * the same three checks `useCheckoutBudget` makes before it offers its silent
   * button. Anything short of all three is no badge and no reordering.
   */
  async function readAllowance(): Promise<void> {
    silent.value = {}
    const buyer = wallet.publicKey.value
    if (!buyer || !status.value?.rates_available) return

    const found: Partial<Record<SupportedCurrency, boolean>> = {}
    for (const quote of status.value.quotes) {
      try {
        if (quote.currency === 'sol') {
          const state = await budget.loadBudget(buyer)
          if (!state) continue
          const session = await peekSessionSigner(state.buyer)
          const ready = Boolean(session && session.address === state.sessionKey)
          found.sol = ready && !(await budget.silentPaymentObstacle(state, quote.amount))
          continue
        }

        const mint = mintForCurrency(quote.currency, cluster)
        if (!mint) continue
        const state = await budget.loadTokenBudget(buyer, mint)
        if (!state) continue
        const session = await peekSessionSigner(state.buyer)
        const ready = Boolean(session && session.address === state.sessionKey)
        found[quote.currency] = ready
          && !(await budget.tokenBudgetObstacle(state, quote.amount))
      } catch {
        // A read that fails means no suggestion for that currency, not a broken
        // page: its price is still offered, and the wallet can still pay it.
      }
    }
    silent.value = found
  }

  /** The status, confirming a payment this browser left behind on the way back. */
  async function read(): Promise<void> {
    loading.value = true
    failure.value = ''
    try {
      const pending = api.takePending()
      status.value = pending
        ? await api.confirm(pending).catch(() => api.status())
        : await api.status()
    } catch (error) {
      failure.value = apiErrorMessage(error, t('subscription.loadFailure'))
    } finally {
      loading.value = false
    }
  }

  /** Reads once per page, however many places ask on the same tick. */
  async function load(): Promise<void> {
    inflight ??= (async () => {
      await read()
      await readAllowance()
    })().finally(() => {
      inflight = null
    })
    await inflight
  }

  /** One invoice for the picked currency, then the checkout that pays it. */
  async function pay(currency: SupportedCurrency): Promise<void> {
    if (paying.value) return
    paying.value = currency
    failure.value = ''
    try {
      const invoice = await api.startPayment(currency)
      api.rememberPending(invoice.payment_id)
      await navigateTo(paymentLinkPath(invoice.payload))
    } catch (error) {
      failure.value = apiErrorMessage(error, t('subscription.payFailure'))
    } finally {
      paying.value = ''
    }
  }

  return { status, loading, failure, paying, quotes, covered, active, priceLabel, load, pay }
}
