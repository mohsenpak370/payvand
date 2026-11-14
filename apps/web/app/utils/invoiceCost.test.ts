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
 * - `fee_for` (`helpers.rs`): `amount * fee_bps / 10_000 + fixed_fee_lamports`,
 *   capped at `amount - 1`, with the product taken in `u128`.
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

const sellerPays = (feeBps: number, fixedFeeLamports = 0): FeePolicy =>
  ({ feeBps, fixedFeeLamports, feeMode: 'seller' })
const buyerPays = (feeBps: number, fixedFeeLamports = 0): FeePolicy =>
  ({ feeBps, fixedFeeLamports, feeMode: 'buyer' })

/** `amount > rent + fee`, exactly what `settle_payment` requires. */
const settles = (policy: FeePolicy, rent: number, amount: number): boolean =>
  amount > rent + feeOn(policy, amount)

describe('rents', () => {
  it('matches the plan table on mainnet and the lower devnet rent', () => {
    expect(SOL_RENT).toBe(2_491_680)
    expect(TOKEN_RENT).toBe(2_714_400)
    expect(RECEIPT_RENT).toBe(2_985_840)
    expect(TOKEN_ACCOUNT_RENT).toBe(2_039_280)
    expect(MINIMUM_BALANCE(PAYMENT_SPACE, DEVNET)).toBe(1_818_640)
  })
})

describe('feeOn', () => {
  const policy = sellerPays(200)

  it.each([
    // amount, bps, fixed, fee — the shape of `fee_for`.
    [1_000_000_000, 200, 0, 20_000_000],
    [250_000_000, 200, 0, 5_000_000],
    // `fixed_fee_is_charged_on_top_of_the_rate` in the contract suite.
    [100_000_000, 200, 1_000_000, 3_000_000],
    // Below a basis point: the fee floors to zero rather than rounding up.
    [1, 200, 0, 0],
    [99, 200, 0, 1],
    // A free policy charges nothing, and a fixed fee bigger than the price is
    // capped just short of taking the whole invoice.
    [10_000, 0, 0, 0],
    [10, 10_000, 0, 9],
    [1, 0, 5_000_000, 0],
    [0, 200, 0, 0]
  ])('charges %i at %i bps plus %i as %i', (amount, bps, fixed, expected) => {
    expect(feeOn({ feeBps: bps, fixedFeeLamports: fixed, feeMode: 'seller' }, amount))
      .toBe(expected)
  })

  it('takes a large invoice at the rate', () => {
    // 1,000 SOL at 2%.
    expect(feeOn(policy, 1_000_000_000_000)).toBe(20_000_000_000)
  })

  it('multiplies exactly where a float would lose a lamport', () => {
    // The product here is far past 2^53, so `amount * bps` in a float rounds to
    // an odd number; the program's u128 division floors to this value instead.
    expect(feeOn({ feeBps: 9_354, fixedFeeLamports: 0, feeMode: 'seller' }, 2_060_979_503_424_492))
      .toBe(1_927_840_227_503_269)
  })
})

describe('minimumPayable', () => {
  it.each([
    ['SOL at 2% on mainnet', sellerPays(200), SOL_RENT, 2_542_531],
    // Same policy, devnet rent: the floor follows the cluster, not a constant.
    ['SOL at 2% on devnet', sellerPays(200), MINIMUM_BALANCE(PAYMENT_SPACE, DEVNET), 1_855_756],
    ['a token invoice at 2%', sellerPays(200), TOKEN_RENT, 2_769_796],
    ['SOL at 2% plus a 0.001 SOL fixed fee', sellerPays(200, 1_000_000), SOL_RENT, 3_562_939],
    ['a free policy', sellerPays(0), SOL_RENT, 2_491_681],
    ['a 10% policy', sellerPays(1_000), SOL_RENT, 2_768_534],
    ['an almost total fee', sellerPays(9_999), SOL_RENT, 24_916_800_001]
  ])('finds the floor for %s', (_name, policy, rent, expected) => {
    const floor = minimumPayable(policy, rent)
    expect(floor).toBe(expected)

    // The two assertions the program itself makes around `InvoiceTooSmall`.
    expect(settles(policy, rent, floor)).toBe(true)
    expect(settles(policy, rent, floor - 1)).toBe(false)
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
    invoiceRent: SOL_RENT,
    receiptRent: RECEIPT_RENT
  }

  it('takes the fee out of the seller, leaving the payer with the price', () => {
    const costs = invoiceCosts({ ...base, amount: 1_000_000_000 })

    expect(costs.fee).toBe(20_000_000)
    expect(costs.sellerReceives).toBe(980_000_000)
    expect(costs.payerPays).toBe(1_000_000_000)
    expect(costs.invoiceRent).toBe(SOL_RENT)
    expect(costs.receiptRent).toBe(RECEIPT_RENT)
    expect(costs.networkFee).toBe(NETWORK_FEE_LAMPORTS)
    expect(costs.sellerTokenAccountRent).toBe(0)
    expect(costs.minPayable).toBe(2_542_531)
  })

  it('charges the fee on top when the policy is buyer-paid', () => {
    const costs = invoiceCosts({ ...base, policy: buyerPays(200), amount: 1_000_000_000 })

    expect(costs.payerPays).toBe(1_020_000_000)
    expect(costs.sellerReceives).toBe(1_000_000_000)
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

  it('agrees with the settle the contract suite asserts', () => {
    // `fixed_fee_is_charged_on_top_of_the_rate`: 0.1 SOL less a 3,000,000 fee
    // reaches the seller, and 97,000,000 is exactly what settle.rs checks.
    const costs = invoiceCosts({
      ...base,
      policy: sellerPays(200, 1_000_000),
      amount: 100_000_000
    })
    expect(costs.fee).toBe(3_000_000)
    expect(costs.sellerReceives).toBe(97_000_000)
  })
})
