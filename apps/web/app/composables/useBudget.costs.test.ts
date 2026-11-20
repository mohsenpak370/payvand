import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'
import { tokenFloatNeed, tokenFloatTopUp, tokenOpenCost } from './useBudget'

/**
 * The allowance's two floats are what pay for a seller who has no token account.
 * They are sized for three such payments, not for ever: the budget's 18,000,000
 * refunds 3,939,360 of records a payment, so a fourth needs a refill. The refill
 * is a plain transfer because the program refunds from the budget's lamports and
 * hands the remainder back on close, so the only question is how much.
 */
describe('the token float', () => {
  /** The harness's rent at 6,960 lamports a byte, to the lamport. */
  const rents = { invoice: 2_714_400, receipt: 1_224_960, account: 2_039_280 }
  const fee = 5_000

  it('asks nothing of the budget and the whole peak of the session key', () => {
    // The session key advances both records on top of the seller's account and
    // gets the records back from the pool in the same transaction; the seller's
    // account is the one thing nobody refunds. The budget parks no float at all.
    expect(tokenFloatNeed(rents, fee)).toEqual({
      budget: 0,
      session: rents.invoice + rents.receipt + rents.account + fee
    })
  })

  it('refills only the session key, by the difference that makes the next payment go', () => {
    // A worn-down key: 4,520,960 against the 5,983,640 — both records, the
    // seller's account and the fee — it has to front before the refund comes
    // back. The budget is never part of a refill, however little it holds.
    expect(tokenFloatTopUp(tokenFloatNeed(rents, fee), { budget: 0, session: 4_520_960 }))
      .toEqual({ budget: 0, session: 1_462_680, total: 1_462_680 })
  })

  it('is the same rule the contract harness applies, from the shared vectors', () => {
    // The harness funds real payments from `token_float_top_up` in
    // `tests/settle.rs`, which models this rule because it cannot call it. Both
    // suites read these rows, so a change to one copy without the other fails
    // here instead of leaving a stale comment saying they agree.
    const rows = readFileSync(
      fileURLToPath(
        new URL(
          '../../../contracts/programs/payvand/tests/fixtures/token-float-top-up.txt',
          import.meta.url
        )
      ),
      'utf8'
    )
      .split('\n')
      .map(line => line.trim())
      .filter(line => line && !line.startsWith('#'))
      .map(line => line.split(/\s+/).map(Number))

    expect(rows.length).toBeGreaterThan(0)
    for (const [invoice, receipt, account, fee, budgetHeld, sessionHeld, toBudget, toSession] of rows) {
      const topUp = tokenFloatTopUp(
        tokenFloatNeed({ invoice: invoice!, receipt: receipt!, account: account! }, fee!),
        { budget: budgetHeld!, session: sessionHeld! }
      )

      expect([topUp.budget, topUp.session]).toEqual([toBudget, toSession])
    }
  })

  it('asks for nothing when both floats already cover a payment', () => {
    expect(tokenFloatTopUp(tokenFloatNeed(rents, fee), { budget: 10_000_000, session: 10_000_000 }))
      .toEqual({ budget: 0, session: 0, total: 0 })
    // And nothing for the side that is short of nothing, even if the other is.
    expect(tokenFloatTopUp(tokenFloatNeed(rents, fee), { budget: 3_939_360, session: 1_000 }))
      .toEqual({ budget: 0, session: 5_982_640, total: 5_982_640 })
  })
})

/**
 * `open_token_budget` brings the budget account to its own rent on every call —
 * `wanted = budget_rent` in the handler — and nothing more, so the quote is the
 * rent the account is short. The session key's float is the program's
 * `SESSION_FEE_FLOAT`, which the client copies.
 */
describe('the floats the client copies', () => {
  const constants = readFileSync(
    fileURLToPath(new URL('../../../contracts/programs/payvand/src/constants.rs', import.meta.url)),
    'utf8'
  )
  const rustConst = (name: string) => {
    const digits = new RegExp(`pub const ${name}: u64 = ([0-9_]+);`).exec(constants)?.[1]
    if (!digits) {
      throw new Error(`${name} is not in constants.rs`)
    }
    return Number(digits.replaceAll('_', ''))
  }

  it('are the rents and floats constants.rs holds, read through the quote', () => {
    // A budget that does not exist yet and a key holding nothing quote the whole
    // rent and the whole session float, so the quote is the client's copy of both.
    const first = tokenOpenCost({
      exists: false,
      held: 0,
      budgetRent: 1_788_720,
      sessionHeld: 0,
      fee: 0
    })

    expect(first.rent).toBe(1_788_720)
    expect(first.feeFloat).toBe(rustConst('SESSION_FEE_FLOAT'))
  })
})

describe('tokenOpenCost', () => {
  const RENT = (129 + 128) * 3_480 * 2 // 1,788,720 lamports
  const SESSION_FLOAT = 12_000_000 // SESSION_FEE_FLOAT
  const FEE = 10_000 // NETWORK_FEE_LAMPORTS
  const held = (lamports: number) => ({ budgetRent: RENT, sessionHeld: SESSION_FLOAT, fee: FEE, held: lamports })

  it('prices a first open at the account rent and the session float', () => {
    // `init` leaves the new account holding exactly its rent, so the handler moves
    // nothing on top of it. A fresh session key too: it holds nothing to offset
    // its own float.
    expect(tokenOpenCost({ exists: false, ...held(0), sessionHeld: 0 })).toEqual({
      rent: RENT,
      feeFloat: SESSION_FLOAT,
      fee: FEE,
      total: RENT + SESSION_FLOAT + FEE
    })
  })

  it('asks a budget that holds its rent for nothing beyond the signature', () => {
    // The account no longer carries a float for records, so an open on an
    // existing budget is the fee alone.
    const cost = tokenOpenCost({ exists: true, ...held(RENT) })

    expect(cost.rent).toBe(0)
    expect(cost.total).toBe(FEE)
  })

  it('asks for the shortfall when a rent change left the account below its rent', () => {
    const cost = tokenOpenCost({ exists: true, ...held(RENT - 100_000) })

    expect(cost.rent).toBe(100_000)
    expect(cost.total).toBe(100_000 + FEE)
  })

  it('asks nothing of a budget that holds more than its rent', () => {
    // An over-funded budget keeps what it has; the handler moves nothing there,
    // and neither may the quote.
    expect(tokenOpenCost({ exists: true, ...held(RENT + 5_000_000) }).rent).toBe(0)
  })

  it('adds whatever the session key is short of its own float', () => {
    const cost = tokenOpenCost({ exists: true, ...held(RENT), sessionHeld: 5_000_000 })

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
