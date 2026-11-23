import type { SupportedCurrency } from '../composables/usePaymentApi'

/**
 * The invoice's economics as arithmetic: the program's fee, the smallest amount
 * it will settle, and the split that follows from both.
 *
 * Nothing here reads a chain or a Nuxt context, so `invoiceCost.test.ts` can
 * hold these numbers against the program's own Rust (`fee_for` in `helpers.rs`,
 * the `InvoiceTooSmall` guard in `instructions/sol.rs`) without a validator.
 */

/** `PAYMENT_SPACE`: the temporary account an invoice is created in. */
export const PAYMENT_SPACE = 230
/** `TOKEN_PAYMENT_SPACE`: its token counterpart. */
export const TOKEN_PAYMENT_SPACE = 262
/**
 * `PAYMENT_RECEIPT_SPACE`: the permanent marker of a settled invoice — the
 * invoice it points at plus the `settled_at` replay guard. The payment's own
 * fields are in the `PaymentSettled` event, which is why this is 48 bytes and
 * not 301.
 */
export const PAYMENT_RECEIPT_SPACE = 48
/** An SPL token account. */
export const TOKEN_ACCOUNT_SPACE = 165

/**
 * One signature plus a margin: the cluster charges 5,000 lamports per
 * signature, and the rest covers a priority fee. This is an estimate, not a
 * chain read — nothing settles differently if it is off.
 */
export const NETWORK_FEE_LAMPORTS = 10_000

/**
 * `MAX_SOL_FEE_LAMPORTS`: the most a percentage fee may cost on a SOL invoice —
 * 0.1 SOL. A large payment pays this instead of a slice of itself that keeps
 * growing with the price.
 */
export const MAX_SOL_FEE_LAMPORTS = 100_000_000
/** `MAX_TOKEN_FEE_BASE_UNITS`: the same ceiling in a six-decimal token — 10 USDC
 * or 10 EURC. */
export const MAX_TOKEN_FEE_BASE_UNITS = 10_000_000

/**
 * The part of `Config` a quote needs: the `FeePolicy` the program settles with.
 *
 * The rate is marginal. The part of the amount at or below the currency's
 * boundary pays `feeBpsBelow`; the part above it pays `feeBpsAbove`. A token
 * invoice whose mint is one of the two named currencies also pays that
 * currency's flat, which is the marker's rent the treasury advances on the
 * token paths. `feeMode` is the `Config` field beside the policy.
 */
export interface FeePolicy {
  /** Basis points on the part at or below the boundary. */
  feeBpsBelow: number
  /** Basis points on the part above it. */
  feeBpsAbove: number
  /** The SOL boundary, in lamports (100,000,000 = 0.1 SOL). */
  solTierLamports: number
  /** The token boundary, in base units (10,000,000 = 10 USDC). */
  tokenTierBaseUnits: number
  /** Flat on a USDC invoice, in base units: the marker's rent. */
  flatUsdcBaseUnits: number
  /** Flat on a EURC invoice, in base units. */
  flatEurcBaseUnits: number
  /** The mint `flatUsdcBaseUnits` applies to. */
  usdcMint: string
  /** The mint `flatEurcBaseUnits` applies to. */
  eurcMint: string
  /** `buyer` charges the fee on top of the price; `seller` takes it out. */
  feeMode: 'seller' | 'buyer'
}

/** What one invoice costs the seller, the payer and the protocol. */
export interface InvoiceCosts {
  currency: SupportedCurrency
  /** The price itself, in the currency's base units. */
  amount: number
  /** Protocol fee, in the same base units. */
  fee: number
  /**
   * The part of `fee` that is the flat for this currency: the permanent payment
   * record's rent. Zero on SOL, which takes that rent out of the price instead.
   */
  recordFee: number
  /**
   * True when the record's rent is the whole fee — the payment is small enough
   * that the percentage is below it — so the screen must not label the fee with
   * a percentage it is not charging.
   */
  feeIsFloor: boolean
  /**
   * True when the fee is the program's ceiling — 0.1 SOL, 10 USDC or 10 EURC —
   * because the rate on this price would have been more. The screen must label
   * the fee as the maximum rather than with a percentage it did not charge.
   */
  feeIsCapped: boolean
  feeBps: number
  /** What the seller keeps, after a seller-paid fee. */
  sellerReceives: number
  /** What the buyer is charged on chain: the price, plus the fee in `buyer`. */
  payerPays: number
  /** Lamports: the invoice's rent, returned to the seller when it closes. */
  invoiceRent: number
  /**
   * Lamports: the permanent marker. A SOL seller pays for it (it comes out of
   * what the settle hands them); a token seller cannot be charged lamports, so
   * the treasury funds that one.
   */
  receiptRent: number
  /** Lamports: the network fee estimate. */
  networkFee: number
  /** Lamports: the seller's token account rent, only when it has to be made. */
  sellerTokenAccountRent: number
  /**
   * The treasury has no account for this mint, so the fee has nowhere to go and
   * no invoice in the currency can settle. Proving that belongs to the caller,
   * which is the only place that reads the chain.
   */
  treasuryTokenAccountMissing: boolean
  /** The smallest amount the program accepts, in base units. */
  minPayable: number
}

/**
 * The rate and flat that apply to an amount in one currency: the boundary and
 * the flat are the program's, selected by the invoice's mint — no `mint` is a
 * SOL invoice, and a token mint the policy does not name pays no flat.
 */
function tierFor(
  policy: FeePolicy,
  mint?: string
): { boundary: number, bpsBelow: number, bpsAbove: number, flat: number } {
  if (!mint) {
    return {
      boundary: policy.solTierLamports,
      bpsBelow: policy.feeBpsBelow,
      bpsAbove: policy.feeBpsAbove,
      flat: 0
    }
  }
  const flat = mint === policy.usdcMint
    ? policy.flatUsdcBaseUnits
    : mint === policy.eurcMint ? policy.flatEurcBaseUnits : 0
  return {
    boundary: policy.tokenTierBaseUnits,
    bpsBelow: policy.feeBpsBelow,
    bpsAbove: policy.feeBpsAbove,
    flat
  }
}

/**
 * The rate a price in this currency ends up paying: the one at or below the
 * boundary, or the one above it. What a screen labels the fee with.
 */
export function rateOn(policy: FeePolicy, amount: number, mint?: string): number {
  const { boundary, bpsBelow, bpsAbove } = tierFor(policy, mint)
  return Math.trunc(amount) <= boundary ? bpsBelow : bpsAbove
}

/**
 * `fee_for`: the marginal rate, with the permanent record's rent as its floor,
 * ceilinged at 0.1 SOL or 10 token units so a large invoice pays a plausible
 * fee rather than a multiple of the service, and capped at `amount - 1` so a
 * payment can never hand over the whole invoice.
 *
 * The floor is expressed in the currency the seller is charged in. A token
 * seller cannot be charged lamports, so the marker's rent is the currency's flat
 * and the fee is the larger of it and the rate. A SOL seller pays that rent
 * directly out of the price (`settle_payment` hands them `amount - rent - marker
 * rent - fee`), so it is taken off the rate instead and the fee is zero while the
 * rent is the larger of the two. Either way the seller's total is the larger of
 * the rate and the rent.
 *
 * The program multiplies in `u128`; the BigInt product is the same guarantee, and
 * stays exact for every amount a JavaScript number can hold.
 */
export function feeOn(
  policy: FeePolicy,
  amount: number,
  mint: string | undefined,
  receiptRent: number
): number {
  return feeCalculation(policy, amount, mint, receiptRent).fee
}

/**
 * `feeOn`, plus whether the ceiling is what set the fee. The two answers come
 * from one calculation so the amount a screen prints and the fact it labels it
 * with can never disagree.
 */
function feeCalculation(
  policy: FeePolicy,
  amount: number,
  mint: string | undefined,
  receiptRent: number
): { fee: number, capped: boolean } {
  // The chain holds whole base units, and a price can reach here with a
  // fraction on it (a typed amount times decimals, a percentage). Truncating
  // keeps the quote on the low side of the wire, and it keeps `BigInt` — which
  // throws on a fraction — out of a crash a buyer would see as a failed
  // payment. `NaN` and `Infinity` are not payments either.
  const units = Math.trunc(amount)
  if (!Number.isFinite(units) || units <= 0) return { fee: 0, capped: false }

  const { boundary, bpsBelow, bpsAbove, flat } = tierFor(policy, mint)
  const total = BigInt(units)
  const edge = BigInt(Math.max(0, Math.trunc(boundary)))
  const below = total < edge ? total : edge
  const above = total - below
  const proportional = (below * BigInt(bpsBelow) + above * BigInt(bpsAbove)) / 10_000n
  const rent = BigInt(Math.max(0, Math.trunc(receiptRent)))
  const fee = mint === undefined
    ? proportional - (proportional < rent ? proportional : rent)
    : proportional > BigInt(Math.max(0, Math.trunc(flat)))
      ? proportional
      : BigInt(Math.max(0, Math.trunc(flat)))
  // The ceiling comes first, as in the program: the rate, the rent and the flat
  // can only read the fee down from here.
  const ceiling = BigInt(mint === undefined ? MAX_SOL_FEE_LAMPORTS : MAX_TOKEN_FEE_BASE_UNITS)
  const limited = fee < ceiling ? fee : ceiling
  const cap = BigInt(units - 1)
  return {
    fee: Number(limited < cap ? limited : cap),
    // Only the ceiling counts as "capped": `amount - 1` is a safety net that a
    // real policy never leans on, and calling it the maximum would mislabel it.
    capped: fee > ceiling
  }
}

/**
 * The smallest amount the program settles: `amount > rent + fee`, the
 * `InvoiceTooSmall` guard.
 *
 * `amount - fee(amount)` never falls while every rate is below 100%, so a
 * doubling search for a settling amount followed by a bisection lands on the
 * first one. `Infinity` means the policy admits no payment at all — at 10,000
 * bps the fee is always `amount - 1`, so `amount > rent + amount - 1` is never
 * true above a zero rent.
 */
export function minimumPayable(
  policy: FeePolicy,
  rent: number,
  mint: string | undefined,
  receiptRent: number
): number {
  const floor = Math.max(0, Math.trunc(rent))
  const settles = (amount: number) => amount > floor + feeOn(policy, amount, mint, receiptRent)

  let low = 0
  let high = 1
  while (high <= Number.MAX_SAFE_INTEGER && !settles(high)) high *= 2
  if (!settles(high)) return Number.POSITIVE_INFINITY

  while (high - low > 1) {
    const middle = Math.floor((low + high) / 2)
    if (settles(middle)) high = middle
    else low = middle
  }
  return high
}

export interface InvoiceQuoteInput {
  policy: FeePolicy
  /** The price, in the currency's base units. */
  amount: number
  currency: SupportedCurrency
  /** The invoice's mint, which selects the token flat. Absent on SOL. */
  mint?: string
  invoiceRent: number
  receiptRent: number
  /** Lamports the payer must add because the seller has no token account yet. */
  sellerTokenAccountRent?: number
  /** Read on chain by the caller: the treasury's account for this mint. */
  treasuryTokenAccountMissing?: boolean
}

/** The whole quote, from a policy and rents the caller already read. */
export function invoiceCosts(input: InvoiceQuoteInput): InvoiceCosts {
  const { policy } = input
  // Same rule as `feeOn`: the quote reports the amount the wire will carry.
  const amount = Number.isFinite(input.amount) ? Math.trunc(input.amount) : 0
  // A SOL invoice has no mint; a token one without a known mint still uses the
  // token boundary but matches neither currency, so it pays no flat.
  const mint = input.currency === 'sol' ? undefined : (input.mint ?? '')
  const receiptRent = Math.trunc(input.receiptRent)
  const flat = Math.max(0, Math.trunc(tierFor(policy, mint).flat))
  const { fee, capped } = feeCalculation(policy, amount, mint, receiptRent)
  // The flat inside `fee`, after the `amount - 1` cap: shown apart from the
  // percentage so the seller can see both parts.
  const recordFee = Math.min(flat, fee)
  // The record is the whole fee when the flat is what the fee comes to — on a
  // token invoice, or when the SOL fee has been swallowed by the rent it is
  // measured against.
  const feeIsFloor = mint === undefined ? fee === 0 : flat > 0 && fee <= flat
  const sellerPays = policy.feeMode === 'seller'
  // The invoice rent is a loan, not a cost: the account closes to the seller in
  // every SOL settle. The marker's rent is a cost, and on a SOL invoice it is
  // the seller's — `settle_payment` hands them `amount - rent - marker rent -
  // fee` and only returns the invoice rent on close. A token settle cannot
  // charge a seller lamports, so there the treasury funds the marker and the
  // seller keeps `amount - fee`.
  //
  // Either way the buyer pays the price: a SOL buyer advances the marker's rent
  // at `init` and is made whole through the seller's reduced proceeds.
  const markerRent = input.currency === 'sol' ? receiptRent : 0
  const netOfFee = sellerPays ? amount - fee : amount

  return {
    currency: input.currency,
    amount,
    fee,
    recordFee,
    feeIsFloor,
    feeIsCapped: capped,
    feeBps: rateOn(policy, amount, mint),
    sellerReceives: Math.max(0, netOfFee - markerRent),
    payerPays: sellerPays ? amount : amount + fee,
    invoiceRent: input.invoiceRent,
    receiptRent: input.receiptRent,
    networkFee: NETWORK_FEE_LAMPORTS,
    sellerTokenAccountRent: Math.trunc(input.sellerTokenAccountRent ?? 0),
    treasuryTokenAccountMissing: input.treasuryTokenAccountMissing === true,
    // Only a SOL settle takes its rents out of the price (`amount > invoice rent
    // + marker rent + fee`). A token settle refunds the records from the
    // treasury and asks only that the seller receive something (`amount > fee`),
    // so its floor carries no rent.
    minPayable: minimumPayable(
      policy,
      input.currency === 'sol' ? input.invoiceRent + markerRent : 0,
      mint,
      receiptRent
    )
  }
}
