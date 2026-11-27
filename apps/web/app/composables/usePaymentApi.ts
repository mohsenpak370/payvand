import type { FiatCurrency } from './useFiat'

export type PaymentStatus = 'pending' | 'settled' | 'missing'

export type SupportedCurrency = 'sol' | 'usdc' | 'eurc'

/** `GET /api/config`: everything a quote needs, from one cached read. */
export interface ConfigResponse {
  cluster: string
  program_id: string
  admin: string
  fee_mode: 'seller' | 'buyer'
  /** Lamports `open_budget` parks on a session key. */
  session_fee_float: number
  fees: {
    fee_bps_below: number
    fee_bps_above: number
    sol_tier_lamports: number
    token_tier_base_units: number
    flat_usdc_base_units: number
    flat_eurc_base_units: number
    network_fee_lamports: number
    network_fee_usdc_base_units: number
    network_fee_eurc_base_units: number
  }
  mints: { usdc: string, eurc: string }
  /** Account sizes the rent minimums are quoted for. */
  spaces: Record<string, number>
  /** `getMinimumBalanceForRentExemption` for each size in `spaces`. */
  rent: Record<string, number>
}

/**
 * One way to pay an invoice: a currency, its price, and the address that price
 * is bound to. A seller who takes SOL and USDC puts two of these on one code,
 * and the buyer picks on their own screen.
 */
export interface PaymentOption {
  currency: SupportedCurrency
  /** The token mint; empty for native SOL. */
  mint: string
  /** 9 for SOL, 6 for the stablecoins. */
  decimals: number
  /** Price in the currency's base units. */
  amount: number
  payment_pda: string
}

export interface PaymentRecord {
  id: string
  status: PaymentStatus
  payment_pda: string
  seller: string
  reference: string
  amount_lamports: number
  /** Every currency this invoice accepts; one entry on a chain read. */
  options?: PaymentOption[]
  memo: string
  /** Empty on records read back from chain, which stores neither field. */
  name: string
  description: string
  /** The receipt, when the invoice was built from a shop. Empty on chain reads. */
  items?: PaymentItem[]
  /** The code attached while creating the invoice; null on chain reads. */
  discount?: PaymentDiscount | null
  /** Whether the buyer may apply one of the seller's codes themselves. */
  allow_discounts?: boolean
  cluster: string
  endpoint: string
  seller_user: string | null
  settled_by: string | null
  created_at_unix: number
  settled_at_unix: number
}

/**
 * One line of a receipt. Priced in lamports — the unit the invoice itself is
 * charged in — with the quantity kept beside it, so the buyer's screen can show
 * "2 × Flat white" rather than a total nobody can check.
 */
export interface PaymentItem {
  name: string
  quantity: number
  /** Unit price, not the line total. */
  amount_lamports: number
}

export interface SponsorInfo {
  /** False when the relayer has no key, is out of budget, or this account hit its cap. */
  available: boolean
  /** The fee payer to name; null when there is no relayer at all. */
  address: string | null
  /** Lamports a wallet needs before it can settle a token invoice itself. */
  needs_lamports: number
  per_wallet_daily: number
}

export interface SponsoredSettleInput {
  /** Base64 wire transaction: fee payer is the sponsor, buyer signature present. */
  transaction: string
  buyer: string
  seller: string
  reference: string
  /** The invoice's mint; empty for a SOL invoice, which the relayer settles
   * with `settle_payment`. */
  mint: string
  amount: number
  memo: string
  /**
   * The shop's `ShopPolicy`, when the invoice was priced by one. The relayer
   * rebuilds the settle message with it as its trailing account, so omitting it
   * for a shop invoice means the rebuild is a slot short and is refused.
   */
  shopPolicy?: string
  cluster: string
  endpoint?: string
}

export interface CreatePaymentInput {
  seller: string
  reference: string
  /** The currencies the seller accepts, each with its own price. */
  options: { currency: SupportedCurrency, amount: number }[]
  memo: string
  name?: string
  description?: string
  /** Receipt lines, when the invoice was built from the seller's shop. */
  items?: PaymentItem[]
  /** Code to bake into the invoice, priced against the seller's own codes. */
  discount_code?: string
  /**
   * Let the buyer pick one of the seller's codes on their own screen. An
   * invoice carries a code or offers the choice, never both.
   */
  allow_discounts?: boolean
  cluster: string
  endpoint?: string
}

export interface CreatePaymentResponse {
  payment: PaymentRecord
  qrPayload: string
}

/**
 * A reusable code: one offer — a price, a currency and the words around it —
 * that any buyer can pay, as often as they like. It is not an invoice: the chain
 * settles an invoice once, so every visit mints a fresh one from this offer.
 */
export interface ReusableLink {
  id: string
  seller: string
  cluster: string
  currency: SupportedCurrency
  /** Price in the currency's base units: lamports, or the token's own units. */
  amount: number
  memo: string
  name: string
  description: string
  /** The path the code opens, with this app's origin in front of it. */
  path: string
  created_at_unix: number
}

/** What a seller posts to store a reusable code. */
export interface CreateReusableLinkInput {
  seller: string
  currency: SupportedCurrency
  amount: number
  memo?: string
  name?: string
  description?: string
  shop_id?: string
  cluster: string
  endpoint?: string
}

/** A payment code kept on the account, filed under a shop, so it opens again. */
export interface SavedPayment {
  id: string
  payment_id: string
  /** The code itself — the invoice lives here, not in the API. */
  payload: string
  /** The shop that keeps it. Codes saved before shops kept them have none. */
  shop_id: string | null
  created_at_unix: number
}

/** Contents of the JSON blob the seller encodes into the QR code. */
export interface PaymentQrPayload {
  v: number
  id: string
  seller: string
  reference: string
  amount_lamports: number
  /** Every currency this invoice accepts. Absent on codes made before it. */
  options?: PaymentOption[]
  memo: string
  name?: string
  description?: string
  /** The receipt, when the seller priced the invoice from their shop. */
  items?: PaymentItem[]
  /** Attached at creation: the buyer's screen applies it without typing. */
  discount?: PaymentDiscount | null
  /** The seller lets the buyer apply one of their codes. */
  allow_discounts?: boolean
  cluster: string
  endpoint: string
  /** The seller's account id, so the buyer's screen can price their codes. */
  seller_user?: string | null
  /**
   * The currency the seller reads prices in, stamped in when the invoice is
   * created, so a buyer anywhere sees the fiat figure in that currency.
   */
  currency?: FiatCurrency | null
  /**
   * The card design the seller chose, also stamped in at creation. Unknown
   * values fall back to the plain card — see `cardDesignOf`.
   */
  design?: string
}

export interface DiscountCode {
  id: string
  code: string
  percent_off: number
}

/** A discount an invoice carries in its own code. */
export interface PaymentDiscount {
  code: string
  percent_off: number
}

export interface ValidateDiscountInput {
  code: string
  /**
   * The seller's wallet address, taken from the payment code. The API resolves
   * the seller's account from it, so the percentage is always theirs.
   */
  seller: string
  price_lamports: number
  /** The wallet asking, so a code already spent by it is refused. */
  buyer?: string
}

export interface DiscountPreview {
  code: string
  percent_off: number
  discounted_lamports: number
  saved_lamports: number
}

/**
 * The shared link opens the checkout, so any phone camera (or chat app) lands
 * on a page that can take the payment. The invoice rides in the path as a
 * token, which keeps the link short enough for a printed code.
 */
export function paymentLinkPath(payload: string): string {
  return `/p/${encodePaymentToken(payload)}`
}

export function paymentUrl(payload: string): string {
  return `${window.location.origin}${paymentLinkPath(payload)}`
}

/**
 * The short reference both sides of a payment can say out loud: the first eight
 * characters of the payment address, uppercased so a code read aloud or typed
 * back never fails on case. The address is in the seller's transaction list and
 * in the buyer's own code, so the two sides compute the same string without
 * asking the API for it.
 *
 * Base58 leaves out 0, O, I and l already; folding the case costs a little room
 * (58^8 becomes roughly 33^8) and buys a code that survives being spoken.
 */
export function referenceCode(id: string): string {
  return id.slice(0, 8).toUpperCase()
}

/**
 * Base64url without padding, so the invoice fits in one URL path segment.
 * The API decodes the same token back into the invoice.
 */
export function encodePaymentToken(payload: string): string {
  let binary = ''
  for (const byte of new TextEncoder().encode(payload)) {
    binary += String.fromCharCode(byte)
  }
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
}

export function decodePaymentToken(token: string): string {
  const base64 = token.replace(/-/g, '+').replace(/_/g, '/')
  const padded = base64.padEnd(base64.length + ((4 - (base64.length % 4)) % 4), '=')
  const binary = atob(padded)
  return new TextDecoder().decode(Uint8Array.from(binary, char => char.charCodeAt(0)))
}

/**
 * What the QR encodes: a Solana Pay transaction request pointing at the API.
 * A wallet recognizes the `solana:` URL, asks the API for a `settle_payment`
 * transaction and signs it — the same instruction the in-app checkout sends.
 * The invoice itself rides in the path, so the code stays stateless.
 */
export function solanaPayUrl(payload: string, apiBase: string): string {
  return `solana:${apiBase.replace(/\/+$/, '')}/api/solana-pay/${encodePaymentToken(payload)}`
}

/**
 * Reduces every accepted form of a payment code — raw JSON, `/p/<token>`, the
 * legacy `?p=` link, a `solana:` request or a bare token — to the JSON the
 * invoice is made of. The QR re-encodes this, so it must be the invoice text
 * itself and never the link it was read from.
 */
export function unwrapQrText(raw: string): string {
  let text = raw.trim()
  // Unwrap a wallet code first: a Solana Pay transaction request carries the
  // same invoice, base64url-encoded in its path.
  if (/^solana:/i.test(text)) {
    try {
      const token = new URL(text.slice(text.indexOf(':') + 1)).pathname
        .split('/')
        .filter(Boolean)
        .pop()
      if (!token) throw new Error()
      text = decodePaymentToken(token)
    } catch {
      throw new Error(appT('errors.unreadableSolanaCode'))
    }
  } else if (/^https?:\/\//i.test(text)) {
    // Unwrap a scanned link; a pasted raw JSON payload still works.
    try {
      const url = new URL(text)
      const embedded = url.searchParams.get('p')
      if (embedded) {
        text = embedded
      } else {
        // `/p/<token>` carries the whole invoice in its last path segment.
        const token = url.pathname.split('/').filter(Boolean).pop()
        if (token && !text.startsWith('{')) text = decodePaymentToken(token)
      }
    } catch {
      // Fall through: the JSON error below explains it to the buyer.
    }
  } else if (!text.startsWith('{')) {
    // A bare token is the same invoice in its URL-safe form, so the short
    // link can be pasted without its origin.
    try {
      const decoded = decodePaymentToken(text)
      if (decoded.startsWith('{')) text = decoded
    } catch {
      // Leave it as-is: the JSON error below explains the problem.
    }
  }
  return text
}

export function parseQrPayload(raw: string): PaymentQrPayload {
  const text = unwrapQrText(raw)

  let parsed: PaymentQrPayload
  try {
    parsed = JSON.parse(text) as PaymentQrPayload
  } catch {
    // JSON.parse's own message ("Unexpected token …") means nothing to buyers.
    throw new Error(appT('errors.notPaymentCode'))
  }
  if (parsed?.v !== 1 || !parsed.id || !parsed.seller || !parsed.reference) {
    throw new Error(appT('errors.notOurCode'))
  }
  return parsed
}

/**
 * Every way an invoice can be paid. A code made before currencies existed
 * carries one SOL price and one address, so it reads as a single option and
 * everything downstream only ever deals with options.
 */
export function paymentOptions(payload: PaymentQrPayload): PaymentOption[] {
  if (payload.options?.length) return payload.options
  return [
    {
      currency: 'sol',
      mint: '',
      decimals: 9,
      amount: payload.amount_lamports,
      payment_pda: payload.id
    }
  ]
}

/** `0.05`, `5.00` — a token amount with its own decimals, no trailing zeros. */
export function formatTokenAmount(amount: number, decimals: number): string {
  const whole = Math.floor(amount / 10 ** decimals)
  const fraction = String(amount % 10 ** decimals).padStart(decimals, '0').replace(/0+$/, '')
  return fraction ? `${whole}.${fraction}` : String(whole)
}

/** Nine for SOL, six for the stablecoins — the decimals each one counts in. */
export function currencyDecimals(currency: SupportedCurrency): number {
  return currency === 'sol' ? 9 : 6
}

export function currencySymbol(currency: SupportedCurrency): string {
  if (currency === 'usdc') return 'USDC'
  if (currency === 'eurc') return 'EURC'
  return 'SOL'
}

export function lamportsToSol(lamports: number): string {
  return (lamports / 1_000_000_000).toFixed(9).replace(/\.?0+$/, '')
}

export function solToLamports(sol: number): number {
  return Math.round(sol * 1_000_000_000)
}

export function statusLabel(status: PaymentStatus): string {
  if (status === 'settled') return appT('status.paid')
  // Unpaid invoices have no account at all: settlement creates the invoice
  // account and its receipt together, so `pending` and the old `missing` both
  // mean "nobody has paid this yet".
  return appT('status.awaitingPayment')
}

/** Plain-language network name; devnet/mainnet are jargon to buyers. */
export function networkLabel(cluster: string): string {
  if (cluster === 'localnet') return appT('network.localnet')
  if (cluster === 'devnet') return appT('network.devnet')
  if (cluster === 'testnet') return appT('network.testnet')
  if (cluster === 'mainnet' || cluster === 'mainnet-beta') return appT('network.mainnet')
  return cluster
}

/** `price - percent`, rounded down — the same math the API applies. */
export function discountedLamports(priceLamports: number, percentOff: number): number {
  const percent = Math.min(Math.max(Math.trunc(percentOff), 0), 99)
  return Math.floor((priceLamports * (100 - percent)) / 100)
}

/** Thin wrapper around the Axum API. */
export function usePaymentApi() {
  const config = useRuntimeConfig()
  const base = config.public.apiBase

  async function createPayment(body: CreatePaymentInput): Promise<CreatePaymentResponse> {
    return await $fetch(`${base}/api/payments`, {
      method: 'POST',
      body,
      headers: authHeaders()
    })
  }

  /**
   * Stores an offer the account can be paid for again and again. The offer is
   * all that is made here; the invoices come one per visitor.
   */
  async function createReusableLink(body: CreateReusableLinkInput): Promise<ReusableLink> {
    return await $fetch(`${base}/api/reusable`, {
      method: 'POST',
      body,
      headers: authHeaders()
    })
  }

  /** This visit's own invoice for a reusable code. The buyer needs no account. */
  async function reusableInvoice(id: string): Promise<CreatePaymentResponse> {
    return await $fetch(`${base}/api/reusable/${encodeURIComponent(id)}/invoice`, {
      method: 'POST'
    })
  }

  async function getPayment(
    id: string,
    params: { cluster: string, endpoint?: string }
  ): Promise<PaymentRecord> {
    return await $fetch(`${base}/api/payments/${id}`, { params })
  }

  /**
   * Files a payment code under one of the account's shops; saving the same
   * payment twice is a no-op that answers with the entry already kept.
   */
  async function savePayment(payload: string, shopId?: string): Promise<SavedPayment> {
    return await $fetch(`${base}/api/saved-payments`, {
      method: 'POST',
      body: { payload, shop_id: shopId ?? null },
      headers: authHeaders()
    })
  }

  async function listSavedPayments(): Promise<SavedPayment[]> {
    return await $fetch(`${base}/api/saved-payments`, { headers: authHeaders() })
  }

  /** Many payment addresses in one round trip — used by the seller screen. */
  async function watchPayments(
    pdas: string[],
    params: { cluster: string, endpoint?: string }
  ): Promise<PaymentRecord[]> {
    return await $fetch(`${base}/api/payments/watch`, {
      method: 'POST',
      body: { pdas, ...params }
    })
  }

  async function listDiscounts(): Promise<DiscountCode[]> {
    return await $fetch(`${base}/api/discounts`, { headers: authHeaders() })
  }

  async function createDiscount(body: {
    code: string
    percent_off: number
  }): Promise<DiscountCode> {
    return await $fetch(`${base}/api/discounts`, {
      method: 'POST',
      body,
      headers: authHeaders()
    })
  }

  async function deleteDiscount(id: string): Promise<{ ok: boolean }> {
    return await $fetch(`${base}/api/discounts/${id}`, {
      method: 'DELETE',
      headers: authHeaders()
    })
  }

  /** Prices an invoice with a code — what the buyer's screen shows. */
  async function validateDiscount(
    body: ValidateDiscountInput
  ): Promise<DiscountPreview> {
    return await $fetch(`${base}/api/discounts/validate`, {
      method: 'POST',
      body,
      headers: authHeaders()
    })
  }

  /**
   * Records that a buyer spent a code, once the invoice they paid has settled.
   * Only the seller's own session may: the API reads the buyer off the
   * settlement, and a stranger must not be able to burn someone else's code.
   */
  async function recordDiscountUse(body: {
    code: string
    seller: string
    invoice: string
  }): Promise<{ recorded: boolean }> {
    return await $fetch(`${base}/api/discounts/use`, {
      method: 'POST',
      body,
      headers: authHeaders()
    })
  }

  /**
   * Whether a relayer will pay this payment's network fee and its two record
   * rents, and the address to name as the fee payer when it will. Every token
   * payment asks: Payvand's fee covers the network fee, so the payer needs no
   * SOL of their own.
   */
  async function getSponsor(wallet?: string): Promise<SponsorInfo> {
    // The wallet names whose daily allowance this is: a buyer without an account
    // has the same cap, counted against the address that signs.
    const query = wallet ? `?wallet=${encodeURIComponent(wallet)}` : ''
    return await $fetch(`${base}/api/sponsor${query}`, { headers: authHeaders() })
  }

  /**
   * Hands the buyer-signed transaction to the relayer. The transaction's fee
   * payer is the sponsor and its message is the one the relayer rebuilds from
   * these same fields; anything else is refused without a signature.
   */
  async function sponsorSettle(
    body: SponsoredSettleInput
  ): Promise<{ signature: string }> {
    return await $fetch(`${base}/api/sponsor/settle`, {
      method: 'POST',
      body,
      headers: authHeaders()
    })
  }

  /**
   * The fee policy, the mint addresses and the cluster's rent minimums in one
   * cached answer. A quote reads this instead of the config account and six rent
   * reads of its own: the API keeps it for a minute and shares it with every
   * tab, and the CDN keeps it for another.
   */
  async function getConfig(cluster?: string): Promise<ConfigResponse> {
    const query = cluster ? `?cluster=${encodeURIComponent(cluster)}` : ''
    return await $fetch(`${base}/api/config${query}`)
  }

  return {
    getConfig,
    createPayment,
    createReusableLink,
    reusableInvoice,
    getPayment,
    getSponsor,
    sponsorSettle,
    savePayment,
    listSavedPayments,
    watchPayments,
    listDiscounts,
    createDiscount,
    deleteDiscount,
    validateDiscount,
    recordDiscountUse
  }
}
