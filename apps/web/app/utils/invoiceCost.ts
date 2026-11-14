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
/** `PAYMENT_RECEIPT_SPACE`: the permanent record of a settled invoice. */
export const PAYMENT_RECEIPT_SPACE = 301
/** An SPL token account. */
export const TOKEN_ACCOUNT_SPACE = 165

/**
 * One signature plus a margin: the cluster charges 5,000 lamports per
 * signature, and the rest covers a priority fee. This is an estimate, not a
 * chain read — nothing settles differently if it is off.
 */
export const NETWORK_FEE_LAMPORTS = 10_000

/** The part of `Config` a quote needs. */
export interface FeePolicy {
  /** Seller-side fee in basis points (200 = 2%). */
  feeBps: number
  fixedFeeLamports: number
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
  feeBps: number
  /** What the seller keeps, after a seller-paid fee. */
  sellerReceives: number
  /** What the buyer is charged on chain: the price, plus the fee in `buyer`. */
  payerPays: number
  /** Lamports: the invoice's rent, returned to the seller when it closes. */
  invoiceRent: number
  /** Lamports: the permanent record, funded and refunded by the treasury. */
  receiptRent: number
  /** Lamports: the network fee estimate. */
  networkFee: number
  /** Lamports: the seller's token account rent, only when it has to be made. */
  sellerTokenAccountRent: number
  /**
   * Lamports of record rent the buyer fronts on a token invoice: the invoice
   * account (which the seller receives when it closes) and the permanent
   * receipt. Zero on a SOL invoice, where the treasury refunds the receipt and
   * the seller's share already accounts for the invoice rent.
   */
  buyerRentShare: number
  /**
   * Lamports the buyer pays on top of the price: every cost row above, added.
   * The settle should cost exactly this more than `payerPays`.
   */
  buyerExtraCost: number
  /** The smallest amount the program accepts, in base units. */
  minPayable: number
}

/**
 * `fee_for`: the rate plus the fixed fee, capped at `amount - 1` so a payment
 * can never hand over the whole invoice. The program multiplies in `u128`; the
 * BigInt product is the same guarantee, and stays exact for every amount a
 * JavaScript number can hold.
 */
export function feeOn(policy: FeePolicy, amount: number): number {
  // The chain holds whole base units, and a price can reach here with a
  // fraction on it (a typed amount times decimals, a percentage). Truncating
  // keeps the quote on the low side of the wire, and it keeps `BigInt` — which
  // throws on a fraction — out of a crash a buyer would see as a failed
  // payment. `NaN` and `Infinity` are not payments either.
  const units = Math.trunc(amount)
  if (!Number.isFinite(units) || units <= 0) return 0
  const proportional = (BigInt(units) * BigInt(policy.feeBps)) / 10_000n
  const fee = proportional + BigInt(policy.fixedFeeLamports)
  const cap = BigInt(units - 1)
  return Number(fee < cap ? fee : cap)
}

/**
 * The smallest amount the program settles: `amount > rent + fee`, the
 * `InvoiceTooSmall` guard. Solved rather than probed, so it costs the same at
 * 1% and at 99.99%; the closed form lands within a lamport or two (the floor
 * and the cap move it), and the two corrections below settle the rest.
 *
 * `Infinity` means the policy admits no payment at all: at 10,000 bps the fee is
 * always `amount - 1`, so `amount > rent + amount - 1` is never true.
 */
export function minimumPayable(policy: FeePolicy, rent: number): number {
  if (policy.feeBps >= 10_000) return Number.POSITIVE_INFINITY
  const scaled = BigInt(rent + policy.fixedFeeLamports - 1) * 10_000n
  const scale = BigInt(10_000 - policy.feeBps)
  // ceil(scaled / scale), in integers.
  let candidate = Number(scaled / scale + (scaled % scale ? 1n : 0n))
  while (candidate > 1 && candidate - 1 > rent + feeOn(policy, candidate - 1)) {
    candidate--
  }
  while (candidate <= rent + feeOn(policy, candidate)) {
    candidate++
  }
  return candidate
}

export interface InvoiceQuoteInput {
  policy: FeePolicy
  /** The price, in the currency's base units. */
  amount: number
  currency: SupportedCurrency
  invoiceRent: number
  receiptRent: number
  /** Lamports the payer must add because the seller has no token account yet. */
  sellerTokenAccountRent?: number
}

/** The whole quote, from a policy and rents the caller already read. */
export function invoiceCosts(input: InvoiceQuoteInput): InvoiceCosts {
  const { policy } = input
  // Same rule as `feeOn`: the quote reports the amount the wire will carry.
  const amount = Number.isFinite(input.amount) ? Math.trunc(input.amount) : 0
  const fee = feeOn(policy, amount)
  const sellerPays = policy.feeMode === 'seller'
  // `settle_payment` reimburses the receipt rent from the treasury and pays the
  // seller `amount - rent - fee` before the invoice's rent reaches them, so a
  // SOL buyer loses neither record. `settle_token_payment` has no treasury
  // account in this build, so its payer funds both records themselves; when
  // Phase 3 gives the token path a treasury, this share drops to zero.
  const buyerRentShare = input.currency === 'sol'
    ? 0
    : Math.trunc(input.invoiceRent) + Math.trunc(input.receiptRent)
  const sellerTokenAccountRent = Math.trunc(input.sellerTokenAccountRent ?? 0)

  return {
    currency: input.currency,
    amount,
    fee,
    feeBps: policy.feeBps,
    sellerReceives: sellerPays ? amount - fee : amount,
    payerPays: sellerPays ? amount : amount + fee,
    invoiceRent: input.invoiceRent,
    receiptRent: input.receiptRent,
    networkFee: NETWORK_FEE_LAMPORTS,
    sellerTokenAccountRent,
    buyerRentShare,
    buyerExtraCost: NETWORK_FEE_LAMPORTS + sellerTokenAccountRent + buyerRentShare,
    minPayable: minimumPayable(policy, input.invoiceRent)
  }
}
