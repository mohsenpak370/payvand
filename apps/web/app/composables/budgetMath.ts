import {
  AccountRole,
  address,
  getAddressEncoder,
  getProgramDerivedAddress,
  type Address,
  type Instruction
} from '@vue-solana/nuxt/kit'
import { PAYVAND_PROGRAM_ID } from './usePaymentProgram'

const BUDGET_SEED = new TextEncoder().encode('budget')
const SYSTEM_PROGRAM_ID = address('11111111111111111111111111111111')
/** A token budget per buyer per mint, under its own seed prefix. */
const TOKEN_BUDGET_SEED = new TextEncoder().encode('token-budget')

/** Mirrors `BUDGET_SPACE` in the program: 8 + 32 + 32 + 8 + 8 + 8 + 1. */
export const BUDGET_SPACE = 97
/**
 * Mirrors the program's `SESSION_FEE_FLOAT`. `open_budget` reads the live value
 * from `Config` instead, so this only covers the token flows, which the program
 * still funds from its constant (Phase 3 moves them to `Config` too).
 */
const TOKEN_SESSION_FEE_FLOAT = 12_000_000
/** Mirrors `TOKEN_BUDGET_SPACE`: 8 + 32 + 32 + 32 + 8 + 8 + 8 + 1. */
export const TOKEN_BUDGET_SPACE = 129

export interface TokenOpenCostInput {
  /** A budget account is already there for this buyer and mint. */
  exists: boolean
  /** Lamports on it; 0 when it does not exist. */
  held: number
  /** Rent for `TOKEN_BUDGET_SPACE`, as the cluster charges it. */
  budgetRent: number
  /** Lamports on the session key that will sign; 0 for a key not made yet. */
  sessionHeld: number
  /** The network fee the signature costs. */
  fee: number
}

/**
 * One `open_token_budget` signature, priced from what was read: what it costs on
 * top of the tokens it moves.
 *
 * The handler brings the budget account to its own rent on every call and takes
 * nothing more than that: since revision 12 the pool refunds the records a
 * session key advances, so the budget no longer carries a float for them. The
 * signature pays the rent the account is short — the whole rent for a first open,
 * the gap after a rent change, nothing for one already at its rent — plus the
 * session key's float.
 */
export function tokenOpenCost(
  input: TokenOpenCostInput
): { rent: number, feeFloat: number, fee: number, total: number } {
  const rent = Math.max(0, input.budgetRent - (input.exists ? input.held : 0))
  const feeFloat = Math.max(0, TOKEN_SESSION_FEE_FLOAT - input.sessionHeld)
  return { rent, feeFloat, fee: input.fee, total: rent + feeFloat + input.fee }
}

/**
 * What one payment needs from the floats.
 *
 * The session key advances both records and the signature before the handler
 * runs, and gets the two records back from the pool in the same transaction, so
 * its side is that peak. The seller's token account is not in the sum: it must
 * already exist — the same rule `settle_token_payment` applies — because its rent
 * would be the buyer's money spent on a cost nobody refunds. The budget side is
 * nothing: the pool funds the records, not a float parked on the budget.
 */
export function tokenFloatNeed(
  rents: { invoice: number, receipt: number },
  fee: number
): { budget: number, session: number } {
  const records = rents.invoice + rents.receipt
  return { budget: 0, session: records + fee }
}

/**
 * What a refill has to move: the difference between that need and what each
 * account holds right now, per side. Both are read live because that is what
 * the program does — it refunds from the budget's lamports, not a stored count,
 * so a plain transfer is the whole refill and the budget hands the rest back
 * when it closes.
 */
export function tokenFloatTopUp(
  need: { budget: number, session: number },
  held: { budget: number, session: number }
): { budget: number, session: number, total: number } {
  const budget = Math.max(0, need.budget - held.budget)
  const session = Math.max(0, need.session - held.session)
  return { budget, session, total: budget + session }
}

/** A buyer's monthly pot for one token, and what its vault holds. */
export interface TokenBudgetState {
  pda: Address
  buyer: Address
  sessionKey: Address
  mint: Address
  /** Ceiling for the month, in the mint's base units. */
  monthlyLimit: number
  spent: number
  monthIndex: number
  /** Lamports on the budget account (rent + the refund float). */
  lamports: number
  /** Tokens sitting in the vault right now. */
  amount: number
}

export interface BudgetState {
  /** The buyer's budget PDA. */
  pda: Address
  buyer: Address
  /** The session key the buyer authorised at open time. */
  sessionKey: Address
  monthlyLimit: number
  spent: number
  monthIndex: number
  /** Total lamports held by the PDA (rent-exempt reserve + remaining pot). */
  lamports: number
}

/**
 * What a transaction the session key signs costs: the cluster's base fee, and
 * this client sets no priority fee on those payments. The sweep is one
 * signature, so reserving exactly this empties the key rather than stranding
 * dust on one this device is about to forget; the quoted
 * `NETWORK_FEE_LAMPORTS` would leave half of itself behind for nothing.
 */
const SESSION_SIGNATURE_FEE = 5_000

/**
 * What a sweep of `balance` lamports actually returns: everything but the one
 * signature's cost the session key has to keep to pay for its own transaction.
 * A key that holds no more than that has nothing to hand back.
 */
export function sweepableLamports(balance: number): number {
  const swept = balance - SESSION_SIGNATURE_FEE
  return swept > 0 ? swept : 0
}

/**
 * The system program's `transfer`, built by hand because the system program is
 * not in our IDL: a 4-byte instruction index and the lamports, little-endian.
 */
export function transferLamportsInstruction(
  from: Address,
  to: Address,
  lamports: number
): Instruction {
  const data = new Uint8Array(12)
  const view = new DataView(data.buffer)
  view.setUint32(0, 2, true)
  view.setBigUint64(4, BigInt(lamports), true)
  return {
    programAddress: SYSTEM_PROGRAM_ID,
    accounts: [
      { address: from, role: AccountRole.WRITABLE_SIGNER },
      { address: to, role: AccountRole.WRITABLE }
    ],
    data
  }
}

/**
 * `year * 12 + month` for right now — a line-for-line mirror of the
 * program's `current_month_index`, so the UI and the chain agree on when a
 * monthly allowance resets.
 */
export function currentMonthIndex(): number {
  const z = Math.floor(Date.now() / 86_400_000) + 719_468
  const era = Math.floor(z / 146_097)
  const doe = z - era * 146_097
  const yoe = Math.floor(
    (doe - Math.floor(doe / 1460) + Math.floor(doe / 36_524) - Math.floor(doe / 146_096)) / 365
  )
  const doy = doe - (365 * yoe + Math.floor(yoe / 4) - Math.floor(yoe / 100))
  const mp = Math.floor((5 * doy + 2) / 153)
  const month = mp < 10 ? mp + 3 : mp - 9
  const year = yoe + era * 400 + (month <= 2 ? 1 : 0)
  return year * 12 + month
}

/** Address of the buyer's monthly budget PDA (`["budget", buyer]`). */
export async function deriveBudgetPda(buyer: Address): Promise<Address> {
  const [pda] = await getProgramDerivedAddress({
    programAddress: PAYVAND_PROGRAM_ID,
    seeds: [BUDGET_SEED, getAddressEncoder().encode(buyer)]
  })
  return pda
}

/** `["token-budget", buyer, mint]` — one allowance per buyer per token. */
export async function deriveTokenBudgetPda(buyer: Address, mint: Address): Promise<Address> {
  const encoder = getAddressEncoder()
  const [pda] = await getProgramDerivedAddress({
    programAddress: PAYVAND_PROGRAM_ID,
    seeds: [TOKEN_BUDGET_SEED, encoder.encode(buyer), encoder.encode(mint)]
  })
  return pda
}

/** Lamports still spendable this month (a new month resets the counter). */
export function remainingThisMonth(budget: BudgetState): number {
  const spent = budget.monthIndex === currentMonthIndex() ? budget.spent : 0
  return Math.max(0, budget.monthlyLimit - spent)
}

/** Tokens still spendable this month (a new month resets the counter). */
export function tokenRemainingThisMonth(budget: TokenBudgetState): number {
  const spent = budget.monthIndex === currentMonthIndex() ? budget.spent : 0
  return Math.max(0, budget.monthlyLimit - spent)
}

/**
 * The share of the limit below which the pot counts as running low.
 *
 * A pot that merely holds what an ordinary month spent is not worth a prompt —
 * the nudge is for a refill that would actually change what can be paid today.
 */
export const LOW_POT_SHARE = 0.1

/**
 * Whether an allowance should offer a top-up.
 *
 * The month resets the counter, not the money: on the 1st the ceiling is whole
 * again while the pot still holds only what was left, so a full-looking figure
 * can pay almost nothing. That case, and a pot that is nearly empty mid-month,
 * are the two the nudge covers. `spendable` is the remaining ceiling capped by
 * the pot, which is what a payment can actually draw on.
 */
export function potNeedsTopUp(
  limit: number,
  pot: number,
  spendable: number,
  rolledOver: boolean
): boolean {
  if (limit <= 0 || pot >= limit) return false
  return rolledOver || spendable <= limit * LOW_POT_SHARE
}
