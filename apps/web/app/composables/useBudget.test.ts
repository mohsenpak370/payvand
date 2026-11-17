import { address } from '@vue-solana/nuxt/kit'
import { describe, expect, it } from 'vitest'
import {
  sweepableLamports,
  tokenFloatNeed,
  tokenFloatTopUp,
  tokenOpenCost,
  transferLamportsInstruction
} from './useBudget'

/**
 * Task 4.1 hands the session key's float back before the key is forgotten. The
 * transfer is a plain system-program one — the program cannot make it, because
 * the session key is not a PDA — so the two things worth pinning are the amount
 * it may move and the bytes it moves them with. Getting the amount wrong spends
 * the last lamports the signature needs and the sweep fails; getting the bytes
 * wrong sends the wrong amount somewhere the chain accepts.
 *
 * The sweep is one signature with no priority fee, so the fee is the cluster's
 * base fee — the residue a sweep leaves behind has to be nothing, or the key
 * this device forgets still holds lamports nobody can spend.
 */

const from = address('B8V5fS6rPD9P9Hhw6xYJ2Kf7UEfFQHhBb4HfCxRxJJPb')
const to = address('3rpBzUBbZPXFKhGywCRduD3hrREDjvDtaw2Mos2yUUPx')
/** The cluster's base fee for one signature. */
const BASE_FEE = 5_000

describe('sweepableLamports', () => {
  it('hands back a funded float minus the fee its own signature costs', () => {
    // What `open_budget` leaves on a fresh session key: 0.03 SOL.
    expect(sweepableLamports(30_000_000)).toBe(30_000_000 - BASE_FEE)
    // The key keeps exactly the fee, so the sweep spends its balance to zero.
    expect(30_000_000 - sweepableLamports(30_000_000)).toBe(BASE_FEE)
  })

  it('sweeps nothing from a key that holds only its fee', () => {
    expect(sweepableLamports(BASE_FEE)).toBe(0)
  })

  it('never asks for a negative or impossible transfer', () => {
    // A key worn down to dust, and one already empty: a transfer of 0 is
    // refused by the system program, so neither may produce one.
    expect(sweepableLamports(5_000)).toBe(0)
    expect(sweepableLamports(0)).toBe(0)
  })
})

describe('transferLamportsInstruction', () => {
  it('is the system program transfer, signed by the key that owns the lamports', () => {
    const instruction = transferLamportsInstruction(from, to, 29_990_000)

    expect(instruction.programAddress).toBe('11111111111111111111111111111111')
    expect(instruction.accounts).toEqual([
      { address: from, role: 3 },
      { address: to, role: 1 }
    ])
  })

  it('encodes instruction 2 and the lamports little-endian, as the chain reads them', () => {
    const lamports = 29_990_000
    const data = transferLamportsInstruction(from, to, lamports).data ?? new Uint8Array()
    const view = new DataView(data.buffer, data.byteOffset, data.byteLength)

    expect(data).toHaveLength(12)
    expect(view.getUint32(0, true)).toBe(2)
    expect(view.getBigUint64(4, true)).toBe(BigInt(lamports))
  })
})

/**
 * The allowance's two floats are what pay for a seller who has no token account,
 * and they are spent after a couple of such payments — which is why a third one
 * failed for want of lamports and why the client has to refill them. The refill
 * is a plain transfer because the program refunds from the budget's lamports and
 * hands the remainder back on close, so the only question is how much.
 */
describe('the token float', () => {
  /** The harness's rent at 6,960 lamports a byte, to the lamport. */
  const rents = { invoice: 2_714_400, receipt: 2_985_840, account: 2_039_280 }
  const fee = 5_000

  it('asks the budget for the records and the session key for the seller’s account too', () => {
    // The session key advances all three and gets the records back from the
    // budget; the seller's account is the one thing nobody refunds.
    expect(tokenFloatNeed(rents, fee)).toEqual({
      budget: rents.invoice + rents.receipt,
      session: rents.invoice + rents.receipt + rents.account + fee
    })
  })

  it('refills only the difference, which is what makes the third payment go', () => {
    // What two fresh-seller payments leave behind: the budget's float is gone and
    // the session key is down to 4,520,960 against the 7,744,520 — the three
    // accounts and the fee — it has to front before the refund comes back.
    expect(tokenFloatTopUp(tokenFloatNeed(rents, fee), { budget: 0, session: 4_520_960 }))
      .toEqual({ budget: 5_700_240, session: 3_223_560, total: 8_923_800 })
  })

  it('asks for nothing when both floats already cover a payment', () => {
    expect(tokenFloatTopUp(tokenFloatNeed(rents, fee), { budget: 10_000_000, session: 10_000_000 }))
      .toEqual({ budget: 0, session: 0, total: 0 })
    // And nothing for the side that is short of nothing, even if the other is.
    expect(tokenFloatTopUp(tokenFloatNeed(rents, fee), { budget: 5_700_240, session: 1_000 }))
      .toEqual({ budget: 0, session: 7_743_520, total: 7_743_520 })
  })
})

/**
 * `open_token_budget` brings the budget account to `rent + TOKEN_BUDGET_FEE_FLOAT`
 * on every call — `wanted - held` in the handler — and hands that float back to
 * the session key as invoices settle. The page quotes the same signature, so
 * these numbers are the program's: mainnet rent for `TOKEN_BUDGET_SPACE` (129
 * bytes), and the two floats from `constants.rs`.
 */
describe('tokenOpenCost', () => {
  const RENT = (129 + 128) * 3_480 * 2 // 1,788,720 lamports
  const REFUND_FLOAT = 18_000_000 // TOKEN_BUDGET_FEE_FLOAT
  const SESSION_FLOAT = 12_000_000 // SESSION_FEE_FLOAT
  const FEE = 10_000 // NETWORK_FEE_LAMPORTS
  const held = (lamports: number) => ({ budgetRent: RENT, sessionHeld: SESSION_FLOAT, fee: FEE, held: lamports })

  it('prices a first open at the account rent, the whole float and the session float', () => {
    // `init` leaves the new account holding exactly its rent, so the handler
    // moves the float on top of it.
    // A fresh session key too: it holds nothing to offset its float.
    expect(tokenOpenCost({ exists: false, ...held(0), sessionHeld: 0 })).toEqual({
      rent: RENT,
      refundFloat: REFUND_FLOAT,
      feeFloat: SESSION_FLOAT,
      fee: FEE,
      total: RENT + REFUND_FLOAT + SESSION_FLOAT + FEE
    })
  })

  it('prices a refill at the float its payments spent, not at zero', () => {
    // A budget worn down to 6,000,000 of float: the signature puts back the
    // difference, which is what the page has to show.
    const cost = tokenOpenCost({ exists: true, ...held(RENT + 6_000_000) })

    expect(cost.rent).toBe(0)
    expect(cost.refundFloat).toBe(REFUND_FLOAT - 6_000_000)
    expect(cost.total).toBe(REFUND_FLOAT - 6_000_000 + FEE)
  })

  it('asks a refill for the whole float when the float is gone', () => {
    const cost = tokenOpenCost({ exists: true, ...held(RENT) })

    expect(cost.refundFloat).toBe(REFUND_FLOAT)
    expect(cost.total).toBe(REFUND_FLOAT + FEE)
  })

  it('asks nothing of a budget that still holds its float, or more', () => {
    // Two payments' refunds can leave it above the rent plus the float; the
    // handler moves nothing there, and neither may the quote.
    expect(tokenOpenCost({ exists: true, ...held(RENT + REFUND_FLOAT) }).refundFloat).toBe(0)
    expect(tokenOpenCost({ exists: true, ...held(RENT + REFUND_FLOAT + 5_000_000) }).refundFloat).toBe(0)
  })

  it('adds whatever the session key is short of its own float', () => {
    const cost = tokenOpenCost({ exists: true, ...held(RENT + REFUND_FLOAT), sessionHeld: 5_000_000 })

    expect(cost.feeFloat).toBe(SESSION_FLOAT - 5_000_000)
    expect(cost.total).toBe(SESSION_FLOAT - 5_000_000 + FEE)
  })

  it('prices a key this device has not made yet at the whole session float', () => {
    // `sessionAddress` is optional, and a missing one is quoted as empty rather
    // than at the buyer's own balance.
    const cost = tokenOpenCost({ exists: false, ...held(0), sessionHeld: 0 })

    expect(cost.feeFloat).toBe(SESSION_FLOAT)
  })
})
