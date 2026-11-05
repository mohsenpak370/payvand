export type PaymentStatus = 'pending' | 'settled' | 'missing'

export interface PaymentRecord {
  id: string
  status: PaymentStatus
  payment_pda: string
  seller: string
  reference: string
  amount_lamports: number
  memo: string
  /** Empty on records read back from chain, which stores neither field. */
  name: string
  description: string
  /** The code attached while creating the invoice; null on chain reads. */
  discount?: PaymentDiscount | null
  cluster: string
  endpoint: string
  seller_user: string | null
  settled_by: string | null
  created_at_unix: number
  settled_at_unix: number
}

export interface CreatePaymentInput {
  payment_pda: string
  seller: string
  reference: string
  amount_lamports: number
  memo: string
  name?: string
  description?: string
  /** Code to bake into the invoice, priced against the seller's own codes. */
  discount_code?: string
  cluster: string
  endpoint?: string
}

export interface CreatePaymentResponse {
  payment: PaymentRecord
  qrPayload: string
}

/** A payment code kept on the account, so the profile can open it again. */
export interface SavedPayment {
  id: string
  payment_id: string
  /** The code itself — the invoice lives here, not in the API. */
  payload: string
  created_at_unix: number
}

/** Contents of the JSON blob the seller encodes into the QR code. */
export interface PaymentQrPayload {
  v: number
  id: string
  seller: string
  reference: string
  amount_lamports: number
  memo: string
  name?: string
  description?: string
  /** Attached at creation: the buyer's screen applies it without typing. */
  discount?: PaymentDiscount | null
  cluster: string
  endpoint: string
  /** The seller's account id, so the buyer's screen can price their codes. */
  seller_user?: string | null
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
  seller_user: string
  price_lamports: number
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
      throw new Error('That is a Solana payment code Payvand cannot read.')
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
    throw new Error('That does not look like a Payvand payment code.')
  }
  if (parsed?.v !== 1 || !parsed.id || !parsed.seller || !parsed.reference) {
    throw new Error('That is not a Payvand payment code.')
  }
  return parsed
}

export function lamportsToSol(lamports: number): string {
  return (lamports / 1_000_000_000).toFixed(9).replace(/\.?0+$/, '')
}

export function solToLamports(sol: number): number {
  return Math.round(sol * 1_000_000_000)
}

/** Plain-language label for a payment status badge. */
export function statusLabel(status: PaymentStatus): string {
  if (status === 'settled') return 'Paid'
  // A fresh invoice has no on-chain account until the customer pays — to
  // everyone watching it reads exactly the same as an open invoice.
  return 'Awaiting payment'
}

/** Plain-language network name; devnet/mainnet are jargon to buyers. */
export function networkLabel(cluster: string): string {
  if (cluster === 'localnet') return 'Local test network'
  if (cluster === 'devnet') return 'Test network (devnet)'
  if (cluster === 'testnet') return 'Test network (testnet)'
  if (cluster === 'mainnet' || cluster === 'mainnet-beta') return 'Main network (mainnet)'
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
  const { token } = useAuth()

  /** Creating a payment requires a signed-in account. */
  function authHeaders(): Record<string, string> {
    return token.value ? { authorization: `Bearer ${token.value}` } : {}
  }

  async function createPayment(body: CreatePaymentInput): Promise<CreatePaymentResponse> {
    return await $fetch(`${base}/api/payments`, {
      method: 'POST',
      body,
      headers: authHeaders()
    })
  }

  async function getPayment(
    id: string,
    params: { cluster: string, endpoint?: string }
  ): Promise<PaymentRecord> {
    return await $fetch(`${base}/api/payments/${id}`, { params })
  }

  /** Keeps a payment code on the signed-in account; saving twice is a no-op. */
  async function savePayment(payload: string): Promise<SavedPayment> {
    return await $fetch(`${base}/api/saved-payments`, {
      method: 'POST',
      body: { payload },
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

  return {
    createPayment,
    getPayment,
    savePayment,
    listSavedPayments,
    watchPayments,
    listDiscounts,
    createDiscount,
    deleteDiscount,
    validateDiscount
  }
}
