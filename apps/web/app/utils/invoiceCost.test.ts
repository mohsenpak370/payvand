import { describe, expect, it } from 'vitest'
import {
  MAX_SOL_FEE_LAMPORTS,
  MAX_TOKEN_FEE_BASE_UNITS,
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
 *   currency's tier boundary, `fee_bps_above` on the part above it, with the
 *   permanent record's rent as the floor — the currency's flat on a token
 *   invoice, the rent taken off the rate on a SOL one — and capped at
 *   `amount - 1`, with the products taken in `u128`.
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
const settles = (
  policy: FeePolicy,
  rent: number,
  amount: number,
  mint?: string,
  receiptRent = 0
): boolean => amount > rent + feeOn(policy, amount, mint, receiptRent)

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
    // amount, bps, flat, fee, mint — the shape of `fee_for`, with no lamport
    // rent in play, so the rate is what is charged.
    [1_000_000_000, 200, 0, 20_000_000, undefined],
    [250_000_000, 200, 0, 5_000_000, undefined],
    // A token invoice pays the flat only when the rate is smaller than it.
    [100_000_000, 200, 1_000_000, 2_000_000, USDC],
    [100_000_000, 200, 1_000_000, 2_000_000, EURC],
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
    expect(feeOn(policyOf(bps, { flat }), amount, mint, 0)).toBe(expected)
  })

  it('takes a large invoice at the rate, up to the ceiling', () => {
    // 1,000 SOL at 2% is 20 SOL, past the 0.1 SOL ceiling.
    expect(feeOn(policy, 1_000_000_000_000, undefined, RECEIPT_RENT))
      .toBe(MAX_SOL_FEE_LAMPORTS)
    // Three SOL at 2% is 0.06 SOL, under it: the rate still applies, less the
    // record rent the seller already paid.
    expect(feeOn(policy, 3_000_000_000, undefined, RECEIPT_RENT)).toBe(58_775_040)
  })

  it('never charges more than 0.1 SOL, 10 USDC or 10 EURC', () => {
    // 100 SOL at 2% is 2 SOL of fee; the seller pays 0.1 and no more.
    expect(feeOn(policy, 100_000_000_000, undefined, RECEIPT_RENT))
      .toBe(MAX_SOL_FEE_LAMPORTS)
    // 1,000 USDC at 2% is 20 USDC; ten is the most a token invoice pays.
    expect(feeOn(policy, 1_000_000_000, USDC, RECEIPT_RENT)).toBe(MAX_TOKEN_FEE_BASE_UNITS)
    expect(feeOn(policy, 1_000_000_000, EURC, RECEIPT_RENT)).toBe(MAX_TOKEN_FEE_BASE_UNITS)
    // A mint the policy does not name has the same ceiling: it is the
    // program's, not a property of the two named currencies.
    expect(feeOn(policy, 1_000_000_000, UNNAMED_MINT, RECEIPT_RENT))
      .toBe(MAX_TOKEN_FEE_BASE_UNITS)
    // Under the ceiling nothing changed: 100 USDC at 2% is 2 USDC.
    expect(feeOn(policy, 100_000_000, USDC, RECEIPT_RENT)).toBe(2_000_000)
  })

  it('truncates a fractional amount instead of throwing', () => {
    // A typed price reaches the quote as `Number(input) * 10^decimals`, which
    // can carry a fraction; the chain only holds whole base units.
    expect(feeOn(policy, 1_000.9, undefined, 0)).toBe(feeOn(policy, 1_000, undefined, 0))
    expect(feeOn(policy, 100.999, undefined, 0)).toBe(2)
    expect(feeOn(policy, 0.99, undefined, 0)).toBe(0)
    // Not numbers a payment can be made in, and `BigInt` would throw on both.
    expect(feeOn(policy, Number.NaN, undefined, 0)).toBe(0)
    expect(feeOn(policy, Number.POSITIVE_INFINITY, undefined, 0)).toBe(0)
    expect(feeOn(policy, -1.5, undefined, 0)).toBe(0)
  })

  it('multiplies exactly where a float would lose a lamport', () => {
    // The product here is far past 2^53, so `amount * bps` in a float rounds to
    // an odd number; the program's u128 division floors to this value instead.
    // The ceiling has read the result down by now, so what this pins is that the
    // product is taken exactly before it applies: a float that rounded the fee
    // up would still land on the ceiling, but a policy just under it would not.
    expect(feeOn(policyOf(9_354), 2_060_979_503_424_492, undefined, 0))
      .toBe(MAX_SOL_FEE_LAMPORTS)
    // 106,900,000 lamports at 9,354 bps is 99,994,260: the product is
    // 999,942,600,000, still exact in `f64`, and the fee sits just under the
    // ceiling.
    expect(feeOn(policyOf(9_354), 106_900_000, undefined, 0)).toBe(99_994_260)
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

    expect(feeOn(policy, 99_999_999, undefined, 0)).toBe(999_999)
    expect(feeOn(policy, 100_000_000, undefined, 0)).toBe(1_000_000)
    // One lamport past the boundary is one lamport's worth of rate more.
    expect(feeOn(policy, 100_000_001, undefined, 0)).toBe(1_000_000)
    expect(feeOn(policy, 200_000_000, undefined, 0)).toBe(1_500_000)

    // The token boundary is its own number, and the flat is a floor under it
    // rather than a sum on top.
    expect(feeOn(policy, 10_000_000, USDC, 0)).toBe(100_000)
    expect(feeOn(policy, 20_000_000, USDC, 0)).toBe(150_000)
  })

  it('never charges less than the record costs, in either currency', () => {
    // The rent is 1,224,960 lamports, so 2% of a SOL price crosses it at
    // 61,248,000 lamports.
    // The rate floors to whole lamports, so the first amount whose 2% is one
    // lamport over the rent is 61,248,050, not 61,248,001.
    expect(feeOn(policy, 61_248_049, undefined, RECEIPT_RENT)).toBe(0)
    expect(feeOn(policy, 61_248_050, undefined, RECEIPT_RENT)).toBe(1)
    expect(feeOn(policy, 100_000_000, undefined, RECEIPT_RENT)).toBe(775_040)
    expect(feeOn(policy, 50_000_000, undefined, RECEIPT_RENT)).toBe(0)

    // A token seller pays the flat itself while the rate is below it: this is
    // the record and nothing more.
    const flat = sellerPays(200, 89_400)
    expect(feeOn(flat, 1_000_000, USDC, RECEIPT_RENT)).toBe(89_400)
    expect(feeOn(flat, 4_000_000, USDC, RECEIPT_RENT)).toBe(89_400)
    expect(feeOn(flat, 10_000_000, USDC, RECEIPT_RENT)).toBe(200_000)
  })
})

describe('minimumPayable', () => {
  it.each([
    // The SOL fee is the rate less the marker's rent, which the seller pays
    // separately — so a small invoice owes no fee and the floor is the invoice
    // rent plus one lamport.
    ['SOL at 2% on mainnet', sellerPays(200), SOL_RENT, 2_491_681, undefined],
    // Same policy, devnet rent: the floor follows the cluster, not a constant.
    ['SOL at 2% on devnet', sellerPays(200), MINIMUM_BALANCE(PAYMENT_SPACE, DEVNET), 1_818_641, undefined],
    // No flat: the fee is the rate alone, so the door is the rent plus one.
    ['a token invoice at 2%', sellerPays(200), TOKEN_RENT, 2_714_401, undefined],
    // A SOL invoice pays no flat, so the token flat does not move its floor.
    ['SOL at 2% beside a token flat', sellerPays(200, 1_000_000), SOL_RENT, 2_491_681, undefined],
    // A token settle only asks that the seller receive something, and the
    // `amount - 1` cap makes one base unit settle with no fee at all — the flat
    // is a floor under a fee, not a deposit.
    ['a USDC invoice at 2% plus the marker flat', sellerPays(200, 89_400), 0, 1, USDC],
    ['a free policy', sellerPays(0), SOL_RENT, 2_491_681, undefined],
    ['a 10% policy', sellerPays(1_000), SOL_RENT, 2_491_681, undefined]
  ])('finds the floor for %s', (_name, policy, rent, expected, mint) => {
    const floor = minimumPayable(policy, rent, mint, RECEIPT_RENT)
    expect(floor).toBe(expected)

    // The two assertions the program itself makes around `InvoiceTooSmall`.
    expect(settles(policy, rent, floor, mint, RECEIPT_RENT)).toBe(true)
    expect(settles(policy, rent, floor - 1, mint, RECEIPT_RENT)).toBe(false)
  })

  it('finds a floor at 10,000 bps only because the fee has a ceiling', () => {
    // A whole-invoice rate would take `amount - 1`, and no payment could pass the
    // guard. The 0.1 SOL ceiling stops the fee at 100,000,000, so the door is the
    // rent plus that plus one lamport.
    const policy = sellerPays(10_000)
    const floor = minimumPayable(policy, SOL_RENT, undefined, RECEIPT_RENT)
    expect(floor).toBe(SOL_RENT + MAX_SOL_FEE_LAMPORTS + 1)
    expect(settles(policy, SOL_RENT, floor, undefined, RECEIPT_RENT)).toBe(true)
    expect(settles(policy, SOL_RENT, floor - 1, undefined, RECEIPT_RENT)).toBe(false)
    // Below the ceiling the rate still takes nearly the whole invoice, so a
    // small price admits nothing: three million lamports pays 1,775,040.
    expect(settles(policy, SOL_RENT, 3_000_000, undefined, RECEIPT_RENT)).toBe(false)
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

  it('marks a fee set by the ceiling, and only then', () => {
    // 100 SOL at 2% is 2 SOL of fee; the ceiling makes it 0.1 SOL.
    const sol = invoiceCosts({ ...base, amount: 100_000_000_000 })
    expect(sol.fee).toBe(MAX_SOL_FEE_LAMPORTS)
    expect(sol.feeIsCapped).toBe(true)
    expect(sol.feeIsFloor).toBe(false)
    // The rate is still the one being labelled, so the note can say what the
    // percentage would have come to.
    expect(sol.feeBps).toBe(200)

    // 1,000 USDC at 2% is 20 USDC; ten is the ceiling.
    const usdc = invoiceCosts({
      ...base,
      currency: 'usdc',
      mint: USDC,
      amount: 1_000_000_000
    })
    expect(usdc.fee).toBe(MAX_TOKEN_FEE_BASE_UNITS)
    expect(usdc.feeIsCapped).toBe(true)

    // 5 SOL at 2% is 98,775,040 — under the ceiling, so the rate is what the
    // note must print.
    const under = invoiceCosts({ ...base, amount: 5_000_000_000 })
    expect(under.fee).toBe(98_775_040)
    expect(under.feeIsCapped).toBe(false)
  })

  it('takes the fee out of the seller, leaving the payer with the price', () => {
    const costs = invoiceCosts({ ...base, amount: 1_000_000_000 })

    // 2% is 20,000,000, less the 1,224,960 the seller pays as rent separately.
    expect(costs.fee).toBe(18_775_040)
    expect(costs.feeIsFloor).toBe(false)
    // The seller's total is the rate: the fee plus the rent counted here.
    expect(costs.sellerReceives).toBe(980_000_000)
    expect(costs.payerPays).toBe(1_000_000_000)
    expect(costs.invoiceRent).toBe(SOL_RENT)
    expect(costs.receiptRent).toBe(RECEIPT_RENT)
    expect(costs.networkFee).toBe(NETWORK_FEE_LAMPORTS)
    expect(costs.sellerTokenAccountRent).toBe(0)
    // The floor clears both rents, as `settle_payment` requires, and at that
    // size the fee is nothing.
    expect(costs.minPayable).toBe(3_716_641)
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
    // 2% of 1,000 is 20, under the rent, so the SOL fee is nothing.
    expect(costs.fee).toBe(0)
    expect(costs.feeIsFloor).toBe(true)
    // A price this small cannot settle at all — the marker's rent alone is more
    // than the whole invoice — so the quote floors the seller's share at zero
    // rather than showing a negative number.
    expect(costs.sellerReceives).toBe(0)
    expect(Number.isInteger(costs.sellerReceives)).toBe(true)
  })

  it('charges the fee on top when the policy is buyer-paid', () => {
    const costs = invoiceCosts({ ...base, policy: buyerPays(200), amount: 1_000_000_000 })

    // The fee excludes the rent the seller pays separately: 20,000,000 less
    // 1,224,960.
    expect(costs.payerPays).toBe(1_018_775_040)
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

  it('leaves a token seller and a SOL seller with the same money', () => {
    // `the_record_fee_is_the_floor_of_the_fee` in settle.rs: the token fee is
    // the record's flat, and the SOL fee is the rate less the rent the seller
    // pays directly, so the two totals agree.
    const policy = sellerPays(200, 1_000_000)
    const token = invoiceCosts({ ...base, policy, currency: 'usdc', amount: 100_000_000 })
    const sol = invoiceCosts({ ...base, policy, amount: 100_000_000 })

    expect(token.fee).toBe(2_000_000)
    expect(token.sellerReceives).toBe(98_000_000)
    expect(sol.fee).toBe(775_040)
    expect(sol.sellerReceives).toBe(98_000_000)
  })

  it('labels the fee with the rate unless the record is what is charged', () => {
    const policy = policyOf(0, {
      below: 100,
      above: 50,
      solTier: 100_000_000,
      tokenTier: 10_000_000,
      flat: 89_400
    })

    expect(invoiceCosts({ ...base, policy, amount: 100_000_000 }).feeBps).toBe(100)
    expect(invoiceCosts({ ...base, policy, amount: 100_000_001 }).feeBps).toBe(50)
    // 0.2 SOL: 1% of the first 0.1 SOL plus 0.5% of the second, less the rent.
    expect(invoiceCosts({ ...base, policy, amount: 200_000_000 }).fee)
      .toBe(1_500_000 - RECEIPT_RENT)
    // A 20 USDC invoice pays the rate, which is above its flat.
    expect(invoiceCosts({ ...base, policy, currency: 'usdc', amount: 20_000_000 }).fee)
      .toBe(150_000)

    // A 4 USDC invoice pays the record alone, and the screen must say so rather
    // than labelling it with a percentage nothing is charged at.
    const small = invoiceCosts({ ...base, policy, currency: 'usdc', amount: 4_000_000 })
    expect(small.fee).toBe(89_400)
    expect(small.recordFee).toBe(89_400)
    expect(small.feeIsFloor).toBe(true)
    expect(small.sellerReceives).toBe(4_000_000 - 89_400)
  })
})

describe('the record rent is a floor, not an addition', () => {
  it('keeps the fee above the rent on a token invoice at every size', () => {
    const policy = sellerPays(200, 89_400)
    // Above the `amount - 1` cap's reach, which is 89,401 base units: below that
    // the seller would receive nothing, and the guard refuses the settle anyway.
    for (const amount of [89_401, 4_000_000, 4_470_000, 10_000_000, 1_000_000_000]) {
      expect(feeOn(policy, amount, USDC, RECEIPT_RENT)).toBeGreaterThanOrEqual(89_400)
    }
  })

  it('keeps the seller’s total at the rent on a SOL invoice at every size', () => {
    const policy = sellerPays(200)
    for (const amount of [SOL_RENT + 1, 1_000_000, 61_248_000, 1_000_000_000]) {
      const costs = invoiceCosts({
        policy,
        currency: 'sol',
        amount,
        invoiceRent: SOL_RENT,
        receiptRent: RECEIPT_RENT
      })
      const total = costs.fee + RECEIPT_RENT
      expect(total).toBeGreaterThanOrEqual(RECEIPT_RENT)
      expect(total).toBeGreaterThanOrEqual(Math.trunc(amount * 0.02))
    }
  })

  it('is continuous: one lamport across the crossover costs one lamport more', () => {
    const policy = sellerPays(200)
    const before = invoiceCosts({
      policy,
      currency: 'sol',
      amount: 61_248_049,
      invoiceRent: SOL_RENT,
      receiptRent: RECEIPT_RENT
    })
    const after = invoiceCosts({
      policy,
      currency: 'sol',
      amount: 61_248_050,
      invoiceRent: SOL_RENT,
      receiptRent: RECEIPT_RENT
    })

    expect(before.fee).toBe(0)
    expect(before.feeIsFloor).toBe(true)
    expect(after.fee).toBe(1)
    expect(after.feeIsFloor).toBe(false)
    // The extra lamport of price becomes the first lamport of fee: the seller's
    // net is unchanged, and their total cost moves by exactly one lamport.
    expect(after.sellerReceives).toBe(before.sellerReceives)
    expect(after.fee + after.receiptRent - (before.fee + before.receiptRent)).toBe(1)
  })
})
