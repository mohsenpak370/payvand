import { describe, expect, it } from 'vitest'
import {
  NETWORK_FEE_LAMPORTS,
  PAYMENT_RECEIPT_SPACE,
  PAYMENT_SPACE,
  TOKEN_ACCOUNT_SPACE,
  TOKEN_PAYMENT_SPACE,
  feeOn,
  invoiceCosts,
  minimumPayable,
  type FeePolicy
} from './invoiceCost'

/**
 * The numbers below are the program's, written out from its Rust:
 *
 * - `fee_for` (`helpers.rs`): `fee_bps_below` on the part at or below the
 *   currency's tier boundary, `fee_bps_above` on the part above it, plus the
 *   currency's flat on a token invoice, capped at `amount - 1`, with the
 *   products taken in `u128`.
 * - `Rent::minimum_balance` at each call site in `sol.rs`: `(space + 128) *
 *   lamports_per_byte_year * 2`. Mainnet charges 3,480 lamports per byte-year,
 *   devnet 2,540 — which is why the rents are read from the cluster rather than
 *   copied.
 * - The `InvoiceTooSmall` guard (`sol.rs`): `amount > rent + fee`.
 *
 * The expectations are literals, not calls to the code under test: a fixture
 * that agrees with the helper but not with the program would test nothing.
 */
const MINIMUM_BALANCE = (space: number, lamportsPerByteYear: number): number =>
  (space + 128) * lamportsPerByteYear * 2
const MAINNET = 3_480
const DEVNET = 2_540

const SOL_RENT = MINIMUM_BALANCE(PAYMENT_SPACE, MAINNET)
const TOKEN_RENT = MINIMUM_BALANCE(TOKEN_PAYMENT_SPACE, MAINNET)
const RECEIPT_RENT = MINIMUM_BALANCE(PAYMENT_RECEIPT_SPACE, MAINNET)
const TOKEN_ACCOUNT_RENT = MINIMUM_BALANCE(TOKEN_ACCOUNT_SPACE, MAINNET)

/** The two currencies the policy can name a flat for. */
const USDC = '4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU'
const EURC = 'HzwqbKZw8HxMN6bF2yFZNrht3c2iXXzpKcFu7uBEDKtr'
/** A mint the policy does not name: an invoice in it pays no flat. */
const UNNAMED_MINT = 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v'

/**
 * A policy with a 0 boundary, so every amount pays the one rate and the two
 * marginal rates collapse into the flat rate these cases state. `flat` is what
 * both named currencies charge: the token path's marker rent.
 */
const policyOf = (
  feeBps: number,
  options: {
    flat?: number
    feeMode?: 'seller' | 'buyer'
    below?: number
    above?: number
    solTier?: number
    tokenTier?: number
  } = {}
): FeePolicy => ({
  feeBpsBelow: options.below ?? feeBps,
  feeBpsAbove: options.above ?? feeBps,
  solTierLamports: options.solTier ?? 0,
  tokenTierBaseUnits: options.tokenTier ?? 0,
  flatUsdcBaseUnits: options.flat ?? 0,
  flatEurcBaseUnits: options.flat ?? 0,
  usdcMint: USDC,
  eurcMint: EURC,
  feeMode: options.feeMode ?? 'seller'
})
const sellerPays = (feeBps: number, flat = 0): FeePolicy => policyOf(feeBps, { flat })
const buyerPays = (feeBps: number, flat = 0): FeePolicy =>
  policyOf(feeBps, { flat, feeMode: 'buyer' })

/** `amount > rent + fee`, exactly what a settle requires. */
const settles = (policy: FeePolicy, rent: number, amount: number, mint?: string): boolean =>
  amount > rent + feeOn(policy, amount, mint)

describe('rents', () => {
  it('matches the plan table on mainnet and the lower devnet rent', () => {
    expect(SOL_RENT).toBe(2_491_680)
    expect(TOKEN_RENT).toBe(2_714_400)
    // The marker is 48 bytes: the invoice pointer and the `settled_at` guard.
    expect(RECEIPT_RENT).toBe(1_224_960)
    expect(TOKEN_ACCOUNT_RENT).toBe(2_039_280)
    expect(MINIMUM_BALANCE(PAYMENT_SPACE, DEVNET)).toBe(1_818_640)
  })
})

describe('feeOn', () => {
  const policy = sellerPays(200)

  it.each([
    // amount, bps, flat, fee, mint — the shape of `fee_for`.
    [1_000_000_000, 200, 0, 20_000_000, undefined],
    [250_000_000, 200, 0, 5_000_000, undefined],
    // `the_flat_is_charged_on_the_currencies_the_policy_names` in the contract
    // suite: the flat is the token path's, and SOL pays the rate alone.
    [100_000_000, 200, 1_000_000, 3_000_000, USDC],
    [100_000_000, 200, 1_000_000, 3_000_000, EURC],
    [100_000_000, 200, 1_000_000, 2_000_000, undefined],
    [100_000_000, 200, 1_000_000, 2_000_000, UNNAMED_MINT],
    // Below a basis point: the fee floors to zero rather than rounding up.
    [1, 200, 0, 0, undefined],
    [99, 200, 0, 1, undefined],
    // A free policy charges nothing, and a flat bigger than the price is capped
    // just short of taking the whole invoice.
    [10_000, 0, 0, 0, undefined],
    [10, 10_000, 0, 9, undefined],
    [1, 0, 5_000_000, 0, undefined],
    [0, 200, 0, 0, undefined]
  ])('charges %i at %i bps plus %i as %i on %s', (amount, bps, flat, expected, mint) => {
    expect(feeOn(policyOf(bps, { flat }), amount, mint)).toBe(expected)
  })

  it('takes a large invoice at the rate', () => {
    // 1,000 SOL at 2%.
    expect(feeOn(policy, 1_000_000_000_000)).toBe(20_000_000_000)
  })

  it('truncates a fractional amount instead of throwing', () => {
    // A typed price reaches the quote as `Number(input) * 10^decimals`, which
    // can carry a fraction; the chain only holds whole base units.
    expect(feeOn(policy, 1_000.9)).toBe(feeOn(policy, 1_000))
    expect(feeOn(policy, 100.999)).toBe(2)
    expect(feeOn(policy, 0.99)).toBe(0)
    // Not numbers a payment can be made in, and `BigInt` would throw on both.
    expect(feeOn(policy, Number.NaN)).toBe(0)
    expect(feeOn(policy, Number.POSITIVE_INFINITY)).toBe(0)
    expect(feeOn(policy, -1.5)).toBe(0)
  })

  it('multiplies exactly where a float would lose a lamport', () => {
    // The product here is far past 2^53, so `amount * bps` in a float rounds to
    // an odd number; the program's u128 division floors to this value instead.
    expect(feeOn(policyOf(9_354), 2_060_979_503_424_492))
      .toBe(1_927_840_227_503_269)
  })

  it('changes slope at the tier boundary instead of doubling the fee', () => {
    // `the_tier_boundary_is_marginal` in the contract suite, from the same
    // policy: 1% up to 0.1 SOL, 0.5% above it.
    const policy = policyOf(0, {
      below: 100,
      above: 50,
      solTier: 100_000_000,
      tokenTier: 10_000_000,
      flat: 89_400
    })

    expect(feeOn(policy, 99_999_999)).toBe(999_999)
    expect(feeOn(policy, 100_000_000)).toBe(1_000_000)
    // One lamport past the boundary is one lamport's worth of rate more.
    expect(feeOn(policy, 100_000_001)).toBe(1_000_000)
    expect(feeOn(policy, 200_000_000)).toBe(1_500_000)

    // The token boundary is its own number, and its flat rides on top.
    expect(feeOn(policy, 10_000_000, USDC)).toBe(100_000 + 89_400)
    expect(feeOn(policy, 20_000_000, USDC)).toBe(150_000 + 89_400)
  })
})

describe('minimumPayable', () => {
  it.each([
    ['SOL at 2% on mainnet', sellerPays(200), SOL_RENT, 2_542_531, undefined],
    // Same policy, devnet rent: the floor follows the cluster, not a constant.
    ['SOL at 2% on devnet', sellerPays(200), MINIMUM_BALANCE(PAYMENT_SPACE, DEVNET), 1_855_756, undefined],
    ['a token invoice at 2%', sellerPays(200), TOKEN_RENT, 2_769_796, undefined],
    // A SOL invoice pays no flat, so the token flat does not move its floor.
    ['SOL at 2% beside a token flat', sellerPays(200, 1_000_000), SOL_RENT, 2_542_531, undefined],
    // A token settle only asks that the seller receive something, and the
    // `amount - 1` cap makes one base unit settle with no fee at all.
    ['a USDC invoice at 2% plus the marker flat', sellerPays(200, 89_400), 0, 1, USDC],
    ['a free policy', sellerPays(0), SOL_RENT, 2_491_681, undefined],
    ['a 10% policy', sellerPays(1_000), SOL_RENT, 2_768_534, undefined],
    ['an almost total fee', sellerPays(9_999), SOL_RENT, 24_916_800_001, undefined]
  ])('finds the floor for %s', (_name, policy, rent, expected, mint) => {
    const floor = minimumPayable(policy, rent, mint)
    expect(floor).toBe(expected)

    // The two assertions the program itself makes around `InvoiceTooSmall`.
    expect(settles(policy, rent, floor, mint)).toBe(true)
    expect(settles(policy, rent, floor - 1, mint)).toBe(false)
  })

  it('admits nothing at 10,000 bps, because the fee is always amount - 1', () => {
    const policy = sellerPays(10_000)
    expect(minimumPayable(policy, SOL_RENT)).toBe(Number.POSITIVE_INFINITY)
    for (const amount of [1, SOL_RENT, SOL_RENT * 10, 1_000_000_000_000]) {
      expect(settles(policy, SOL_RENT, amount)).toBe(false)
    }
  })
})

describe('invoiceCosts', () => {
  const base = {
    policy: sellerPays(200),
    currency: 'sol' as const,
    // Ignored on the SOL path; a token case inherits it and its flat is 0 here.
    mint: USDC,
    invoiceRent: SOL_RENT,
    receiptRent: RECEIPT_RENT
  }

  it('takes the fee out of the seller, leaving the payer with the price', () => {
    const costs = invoiceCosts({ ...base, amount: 1_000_000_000 })

    expect(costs.fee).toBe(20_000_000)
    // The SOL seller also carries the marker's rent, which is permanent.
    expect(costs.sellerReceives).toBe(980_000_000 - RECEIPT_RENT)
    expect(costs.payerPays).toBe(1_000_000_000)
    expect(costs.invoiceRent).toBe(SOL_RENT)
    expect(costs.receiptRent).toBe(RECEIPT_RENT)
    expect(costs.networkFee).toBe(NETWORK_FEE_LAMPORTS)
    expect(costs.sellerTokenAccountRent).toBe(0)
    // The floor clears both rents, as `settle_payment` now requires.
    expect(costs.minPayable).toBe(3_792_490)
  })

  it('takes no rent into a token invoice floor, which is the fee alone', () => {
    // `settle_token_payment` refunds the rents from the treasury and asks only
    // that the seller receive something, so `amount > fee` is the whole guard:
    // a 1.5 USDC price must not be measured against the lamport invoice rent.
    const costs = invoiceCosts({ ...base, currency: 'usdc', amount: 1_500_000 })

    expect(costs.minPayable).toBe(1)
    expect(costs.amount).toBeGreaterThanOrEqual(costs.minPayable)
  })

  it('carries the read that says the fee has nowhere to go', () => {
    // The quote cannot see the chain; the caller's read says whether the
    // treasury has an account for the mint, and the checkout holds the button
    // on it rather than letting a settle fail after signing.
    const token = { ...base, currency: 'usdc' as const, amount: 1_500_000 }

    expect(invoiceCosts({ ...token, treasuryTokenAccountMissing: true }).treasuryTokenAccountMissing)
      .toBe(true)
    expect(invoiceCosts(token).treasuryTokenAccountMissing).toBe(false)
  })

  it('reports the whole base units the wire carries, not the fraction', () => {
    const costs = invoiceCosts({ ...base, amount: 1_000.75 })

    expect(costs.amount).toBe(1_000)
    expect(costs.fee).toBe(20)
    // A price this small cannot settle at all — the marker's rent alone is more
    // than the whole invoice — so the quote floors the seller's share at zero
    // rather than showing a negative number.
    expect(costs.sellerReceives).toBe(0)
    expect(Number.isInteger(costs.sellerReceives)).toBe(true)
  })

  it('charges the fee on top when the policy is buyer-paid', () => {
    const costs = invoiceCosts({ ...base, policy: buyerPays(200), amount: 1_000_000_000 })

    expect(costs.payerPays).toBe(1_020_000_000)
    // The buyer buys the fee out, but the marker's rent is still the seller's.
    expect(costs.sellerReceives).toBe(1_000_000_000 - RECEIPT_RENT)
  })

  it('reports the token account rent only when the seller has none', () => {
    const token = {
      ...base,
      currency: 'usdc' as const,
      amount: 50_000_000,
      invoiceRent: TOKEN_RENT
    }

    expect(invoiceCosts({ ...token, sellerTokenAccountRent: TOKEN_ACCOUNT_RENT }))
      .toMatchObject({ sellerTokenAccountRent: TOKEN_ACCOUNT_RENT, invoiceRent: TOKEN_RENT })
    expect(invoiceCosts(token).sellerTokenAccountRent).toBe(0)
  })

  it('charges the buyer the price alone, on both settles', () => {
    // `settle_payment`: the payer advances the invoice and receipt rents, the
    // invoice closes to the seller and the treasury pays the receipt rent back.
    // `settle_token_payment`: the treasury refunds both and closes the invoice
    // to itself. Either way the wire carries `amount`.
    const sol = invoiceCosts({ ...base, amount: 1_000_000_000 })
    expect(sol.payerPays).toBe(1_000_000_000)
    expect(sol.receiptRent).toBe(RECEIPT_RENT)

    const token = invoiceCosts({
      ...base,
      currency: 'usdc',
      amount: 2_500_000,
      invoiceRent: TOKEN_RENT,
      sellerTokenAccountRent: TOKEN_ACCOUNT_RENT
    })
    expect(token.payerPays).toBe(2_500_000)
    // The fee is the seller's, and the missing account is a seller setup step,
    // so neither is added to what the buyer pays.
    expect(token.sellerReceives).toBe(2_450_000)
    expect(token.sellerTokenAccountRent).toBe(TOKEN_ACCOUNT_RENT)
  })

  it('charges the currency flat on a token invoice, as the contract suite does', () => {
    // `the_flat_is_charged_on_the_currencies_the_policy_names` in settle.rs:
    // 2% of 100,000,000 plus the 1,000,000 flat the policy names for USDC.
    const token = invoiceCosts({
      ...base,
      policy: sellerPays(200, 1_000_000),
      currency: 'usdc',
      amount: 100_000_000
    })
    expect(token.fee).toBe(3_000_000)
    // A token seller is charged no lamports of rent, so it all reaches them.
    expect(token.sellerReceives).toBe(97_000_000)

    // The same policy charges a SOL invoice the rate alone.
    const sol = invoiceCosts({
      ...base,
      policy: sellerPays(200, 1_000_000),
      amount: 100_000_000
    })
    expect(sol.fee).toBe(2_000_000)
    expect(sol.sellerReceives).toBe(98_000_000 - RECEIPT_RENT)
  })

  it('labels the fee with the rate the amount ends up paying', () => {
    const policy = policyOf(0, {
      below: 100,
      above: 50,
      solTier: 100_000_000,
      tokenTier: 10_000_000,
      flat: 89_400
    })

    expect(invoiceCosts({ ...base, policy, amount: 100_000_000 }).feeBps).toBe(100)
    expect(invoiceCosts({ ...base, policy, amount: 100_000_001 }).feeBps).toBe(50)
    // 0.2 SOL: 1% of the first 0.1 SOL plus 0.5% of the second.
    expect(invoiceCosts({ ...base, policy, amount: 200_000_000 }).fee)
      .toBe(1_500_000)
    // A 20 USDC invoice keeps the token flat, which the boundary does not cover.
    expect(invoiceCosts({ ...base, policy, currency: 'usdc', amount: 20_000_000 }).fee)
      .toBe(239_400)
  })
})

describe('break-even against the receipt rent', () => {
  /**
   * The smallest amount whose fee covers the receipt rent the treasury funds —
   * `0002`'s break-even rule, `fee_bps × amount ≥ receipt rent`. Searched with
   * `feeOn`, so the answer is the program's arithmetic at the lamport, not a
   * closed form that could drift from the floor and the cap.
   */
  const breakEven = (policy: FeePolicy, receiptRent: number, mint?: string): number => {
    const flat = mint === USDC ? policy.flatUsdcBaseUnits : 0
    if (policy.feeBpsAbove <= 0 && flat <= 0) return Number.POSITIVE_INFINITY
    let low = 1
    // A basis point of the price is always enough to cover any rent this small.
    let high = receiptRent * 10_000
    // The flat is a fixed sum, so the search starts from it rather than from 1.
    low += flat
    high += flat
    while (low < high) {
      const mid = Math.floor((low + high) / 2)
      if (feeOn(policy, mid, mint) >= receiptRent) high = mid
      else low = mid + 1
    }
    return low
  }

  it.each([
    // The marker's rent at each cluster's rate, 48 bytes a receipt.
    ['2% on mainnet', sellerPays(200), RECEIPT_RENT, 61_248_000, undefined],
    ['2% on devnet', sellerPays(200), MINIMUM_BALANCE(PAYMENT_RECEIPT_SPACE, DEVNET), 44_704_000, undefined],
    ['1% on mainnet', sellerPays(100), RECEIPT_RENT, 122_496_000, undefined],
    // The token flat covers the first 1,000,000 base units of rent by itself.
    ['2% plus the USDC flat', sellerPays(200, 1_000_000), RECEIPT_RENT, 11_248_000, USDC]
  ])('breaks even at %s', (_name, policy, rent, expected, mint) => {
    const amount = breakEven(policy, rent, mint)
    expect(amount).toBe(expected)

    // The two sides of the rule, as the program applies it at the lamport.
    expect(feeOn(policy, amount, mint)).toBeGreaterThanOrEqual(rent)
    expect(feeOn(policy, amount - 1, mint)).toBeLessThan(rent)
    // And the durable statement `0002` records, in whole base units: the rate
    // plus the flat on one side, the receipt rent on the other. The flat is
    // counted once, which is why it is scaled rather than multiplied by the
    // amount.
    const flat = mint === USDC ? policy.flatUsdcBaseUnits : 0
    expect(policy.feeBpsAbove * amount + flat * 10_000)
      .toBeGreaterThanOrEqual(rent * 10_000)
    expect(policy.feeBpsAbove * (amount - 1) + flat * 10_000)
      .toBeLessThan(rent * 10_000)
  })

  it('never breaks even on a free policy, so the treasury pays every receipt', () => {
    expect(breakEven(sellerPays(0), RECEIPT_RENT)).toBe(Number.POSITIVE_INFINITY)
  })

  it('is the amount a quote calls zero-profit, not the minimum payable', () => {
    const policy = sellerPays(200)
    const amount = breakEven(policy, RECEIPT_RENT)
    const costs = invoiceCosts({
      policy,
      currency: 'sol',
      amount,
      invoiceRent: SOL_RENT,
      receiptRent: RECEIPT_RENT
    })

    expect(costs.fee).toBe(RECEIPT_RENT)
    expect(costs.minPayable).toBe(3_792_490)
    // Payable long before it is profitable: that gap is `0002`'s documented
    // per-invoice subsidy, and the floor is what the seller feels first.
    expect(costs.minPayable).toBeLessThan(amount)
  })
})
