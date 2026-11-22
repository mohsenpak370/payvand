import type { SupportedCurrency } from './usePaymentApi'

/**
 * The shops subscription: €1.70 for 30 days, priced in SOL, USDC or EURC.
 *
 * The prices are the API's, converted at its cached live rate — the browser
 * never computes them, so the amount shown is the amount the invoice charges.
 * The pending payment id is kept in `localStorage` because paying leaves this
 * page for the checkout: coming back, `confirm()` asks the API to record the
 * period, and the cluster (not the browser) is what proves the invoice settled.
 */

export interface SubscriptionQuote {
  currency: SupportedCurrency
  /** Base units: lamports for SOL, the mint's units for a stablecoin. */
  amount: number
  /** The mint a token payment uses; null for SOL. */
  mint: string | null
}

export interface SubscriptionStatus {
  active: boolean
  /** Unix timestamp of the end of the paid period; 0 when none was ever paid. */
  expires_at_unix: number
  currency: string | null
  amount: number | null
  price_eur_cents: number
  period_days: number
  quotes: SubscriptionQuote[]
  rated_at_unix: number
}

export interface SubscriptionInvoice {
  payment_id: string
  /** The invoice code, in the same shape a seller's QR carries. */
  payload: string
  currency: string
  amount: number
}

/** Where the payment id waits while the seller is in the checkout. */
export const PENDING_SUBSCRIPTION_KEY = 'payvand.subscription-pending.v1'

export function useSubscription() {
  const config = useRuntimeConfig()
  const base = config.public.apiBase
  const { token } = useAuth()

  function authHeaders(): Record<string, string> {
    return token.value ? { authorization: `Bearer ${token.value}` } : {}
  }

  /** The account's paid period and the plan's three prices. */
  async function status(): Promise<SubscriptionStatus> {
    return await $fetch(`${base}/api/subscription`, { headers: authHeaders() })
  }

  /** One invoice for the plan, in the currency the seller picked. */
  async function startPayment(currency: SupportedCurrency): Promise<SubscriptionInvoice> {
    return await $fetch(`${base}/api/subscription/invoice`, {
      method: 'POST',
      body: { currency },
      headers: authHeaders()
    })
  }

  /** Records the period once the invoice has settled on chain. */
  async function confirm(paymentId: string): Promise<SubscriptionStatus> {
    return await $fetch(`${base}/api/subscription/confirm`, {
      method: 'POST',
      body: { payment_id: paymentId },
      headers: authHeaders()
    })
  }

  function rememberPending(paymentId: string): void {
    try {
      localStorage.setItem(PENDING_SUBSCRIPTION_KEY, paymentId)
    } catch {
      // Private mode without storage: the payment still settles, and the
      // invoice can be confirmed from the page it left behind.
    }
  }

  function takePending(): string {
    try {
      const id = localStorage.getItem(PENDING_SUBSCRIPTION_KEY) ?? ''
      if (id) localStorage.removeItem(PENDING_SUBSCRIPTION_KEY)
      return id
    } catch {
      return ''
    }
  }

  return { status, startPayment, confirm, rememberPending, takePending }
}
