import {
  AccountRole,
  address,
  appendTransactionMessageInstruction,
  createTransactionMessage,
  getAddressDecoder,
  getAddressEncoder,
  getBase64EncodedWireTransaction,
  getProgramDerivedAddress,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
  getTransactionDecoder,
  type Address,
  type Instruction,
  type KeyPairSigner
} from '@vue-solana/nuxt/kit'
import {
  PAYVAND_PROGRAM_ID,
  buildInstruction,
  deriveAssociatedTokenAddress,
  accountDiscriminator,
  deriveConfigPda,
  derivePaymentReceiptPda,
  deriveTreasuryPda,
  describeProgramErrorCode
} from './usePaymentProgram'
import type { compileWireTransaction } from './usePaymentProgram'
import { peekSessionSigner } from './useSessionKey'

const BUDGET_SEED = new TextEncoder().encode('budget')
const SYSTEM_PROGRAM_ID = address('11111111111111111111111111111111')
/** A token budget per buyer per mint, under its own seed prefix. */
const TOKEN_BUDGET_SEED = new TextEncoder().encode('token-budget')

/** Mirrors `BUDGET_SPACE` in the program: 8 + 32 + 32 + 8 + 8 + 8 + 1. */
const BUDGET_SPACE = 97
/**
 * Mirrors the program's `SESSION_FEE_FLOAT`. `open_budget` reads the live value
 * from `Config` instead, so this only covers the token flows, which the program
 * still funds from its constant (Phase 3 moves them to `Config` too).
 */
const TOKEN_SESSION_FEE_FLOAT = 10_000_000
/** Mirrors `TOKEN_BUDGET_SPACE`: 8 + 32 + 32 + 32 + 8 + 8 + 8 + 1. */
const TOKEN_BUDGET_SPACE = 129
/** Lamports a token budget keeps to refund the invoice rent it advances. */
const TOKEN_BUDGET_FEE_FLOAT = 10_000_000

/**
 * What one payment to a seller with no token account needs from the two floats.
 * The session key advances the seller's account and both records before the
 * handler runs — it never gets the account back, and the budget hands the
 * records back out of its own float — so it has to hold all three plus the fee
 * it signs its transaction with, while the budget only needs the records.
 */
export function tokenFloatNeed(
  rents: { invoice: number, receipt: number, account: number },
  fee: number
): { budget: number, session: number } {
  const records = rents.invoice + rents.receipt
  return { budget: records, session: records + rents.account + fee }
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

/** Reads and interprets a buyer's budget straight from the cluster. */
export function useBudget() {
  const { client } = useSolanaClient()
  // `open_budget` tops the session key up to `Config::session_fee_float`, so
  // the quote reads the same account the program will.
  const feeConfig = useFeeConfig()

  const rentCache = new Map<number, number>()
  async function rentMinimum(space: number): Promise<number> {
    const hit = rentCache.get(space)
    if (hit !== undefined) return hit
    const lamports = Number(
      await client.rpc.getMinimumBalanceForRentExemption(BigInt(space)).send()
    )
    rentCache.set(space, lamports)
    return lamports
  }

  /** `null` when the buyer has no budget yet. */
  async function loadBudget(buyer: Address): Promise<BudgetState | null> {
    const pda = await deriveBudgetPda(buyer)
    const info = await client.rpc
      .getAccountInfo(pda, { commitment: 'confirmed', encoding: 'base64' })
      .send()
    if (!info.value || !Array.isArray(info.value.data)) return null

    const bytes = Uint8Array.from(atob(info.value.data[0]), c => c.charCodeAt(0))
    const expected = await accountDiscriminator('Budget')
    if (!expected.every((byte, i) => bytes[i] === byte)) {
      throw new Error(appT('errors.notBudgetAccount'))
    }

    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
    return {
      pda,
      buyer: getAddressDecoder().decode(bytes.subarray(8, 40)),
      sessionKey: getAddressDecoder().decode(bytes.subarray(40, 72)),
      monthlyLimit: Number(view.getBigUint64(72, true)),
      spent: Number(view.getBigUint64(80, true)),
      monthIndex: Number(view.getBigUint64(88, true)),
      lamports: Number(info.value.lamports)
    }
  }

  /** Lamports still spendable this month (a new month resets the counter). */
  function remainingThisMonth(budget: BudgetState): number {
    const spent = budget.monthIndex === currentMonthIndex() ? budget.spent : 0
    return Math.max(0, budget.monthlyLimit - spent)
  }

  /** The pot above its rent-exempt reserve — what invoices are actually paid from. */
  async function availablePot(budget: BudgetState): Promise<number> {
    return budget.lamports - (await rentMinimum(BUDGET_SPACE))
  }

  /**
   * What one `open_budget` signature must move out of the buyer's wallet:
   * the pot top-up, the session key's fee float (read from `Config`, the same
   * account the program reads), the budget account's rent (only when it does
   * not exist yet) and the transaction fee. Mirrors the transfers the program
   * performs, so the UI can warn *before* the wallet is asked to sign
   * something that would fail pre-flight.
   */
  async function openBudgetCost(
    buyer: Address,
    monthlyLimit: number,
    /** The key that will sign; without one the float starts from nothing. */
    sessionAddress?: Address
  ): Promise<{ topUp: number, feeFloat: number, rent: number, fee: number, total: number }> {
    const existing = await loadBudget(buyer)
    const rent = existing ? 0 : await rentMinimum(BUDGET_SPACE)
    const pot = existing ? await availablePot(existing) : 0
    const topUp = Math.max(0, monthlyLimit - pot)
    // A key this device has not made yet holds nothing, so `open_budget` funds
    // the whole float; quoting a brand-new key at the buyer's balance would
    // understate what the signature moves.
    const sessionLamports = sessionAddress
      ? Number(
          (await client.rpc.getBalance(sessionAddress, { commitment: 'confirmed' }).send()).value
        )
      : 0
    const { sessionFeeFloat } = await feeConfig.load()
    const feeFloat = Math.max(0, sessionFeeFloat - sessionLamports)
    const fee = NETWORK_FEE_LAMPORTS
    return { topUp, feeFloat, rent, fee, total: topUp + feeFloat + rent + fee }
  }

  /**
   * Runs the compiled transaction through the cluster's simulator (no
   * signature needed) and throws a user-facing message when it would fail.
   * Wallets simulate too — when theirs fails the approval screen turns into
   * a scary "cannot simulate" warning and the send dies with a generic
   * wallet error — so a transaction that reaches the wallet is one that
   * actually succeeds.
   */
  async function preflight(
    wire: Awaited<ReturnType<typeof compileWireTransaction>>
  ): Promise<void> {
    const simulation = await client.rpc
      .simulateTransaction(getBase64EncodedWireTransaction(getTransactionDecoder().decode(wire)), {
        encoding: 'base64',
        commitment: 'confirmed',
        sigVerify: false,
        replaceRecentBlockhash: true
      })
      .send()
    const err = simulation.value.err
    if (err === null) return

    // Kit's RPC hands the custom code back as a bigint — Number() it so
    // the IDL lookup (numeric codes) can match.
    const custom
      = typeof err === 'object'
        && err !== null
        && 'InstructionError' in err
        && Array.isArray(err.InstructionError)
        ? (err.InstructionError[1] as { Custom?: number | bigint } | undefined)?.Custom
        : undefined
    const anchorMessage
      = custom !== undefined ? describeProgramErrorCode(Number(custom)) : null
    if (anchorMessage) {
      throw new Error(anchorMessage)
    }
    // Anchor's built-in errors (ConstraintSeeds, AccountNotInitialized, …)
    // are not in the IDL table, but their message is in the logs.
    const anchorLog = simulation.value.logs?.find(line => line.includes('Error Message:'))
    const loggedMessage = anchorLog?.match(/Error Message: (.+?)\.?$/)?.[1]
    if (loggedMessage) {
      throw new Error(loggedMessage)
    }
    if (simulation.value.logs?.some(line => line.includes('insufficient lamports'))) {
      throw new Error(appT('errors.solForLimit'))
    }
    throw new Error(appT('errors.rejectedOnChain'))
  }

  /**
   * Why the next payment cannot ride the budget, or `null` when it can be
   * settled silently with no wallet popup.
   */
  async function silentPaymentObstacle(
    budget: BudgetState,
    amount: number
  ): Promise<string | null> {
    const session = await peekSessionSigner(budget.buyer)
    if (!session || session.address !== budget.sessionKey) {
      return appT('errors.noSessionKey')
    }
    if (amount > remainingThisMonth(budget)) {
      return appT('errors.limitUsedUp')
    }
    // The pot pays exactly the invoice amount: the session key advances both
    // rents and is reimbursed from the pot, the invoice rent comes back through
    // the close, and the treasury refunds the receipt rent to the pot.
    const needed = amount
    if ((await availablePot(budget)) < needed) {
      return appT('errors.nearlyEmpty')
    }
    return null
  }

  /**
   * Settle an invoice straight from the budget. The session key signs — no
   * wallet is involved, so no popup ever appears. Confirms up to 90s.
   */
  async function sendSilentPayment(
    buyer: Address,
    params: {
      payment: Address
      seller: Address
      reference: Address
      amount: number
      memo: string
    }
  ): Promise<string> {
    const session = await peekSessionSigner(buyer)
    if (!session) {
      throw new Error(appT('errors.noSessionKeyHere'))
    }

    const instruction = buildInstruction(
      'pay_from_budget',
      { amount: params.amount, memo: params.memo },
      {
        session_key: session.address,
        budget: await deriveBudgetPda(buyer),
        payment: params.payment,
        seller: params.seller,
        receipt: await derivePaymentReceiptPda(params.payment),
        reference: params.reference,
        config: await deriveConfigPda(),
        treasury: await deriveTreasuryPda()
      }
    )

    return await submitWithSession(session, instruction)
  }

  /**
   * Signs one instruction with the session key and waits for it to land. No
   * wallet is involved, so no popup ever appears.
   */
  async function submitWithSession(
    session: KeyPairSigner,
    instruction: Instruction
  ): Promise<string> {
    const { value: lifetime } = await client.rpc.getLatestBlockhash().send()
    const message = setTransactionMessageLifetimeUsingBlockhash(
      lifetime,
      setTransactionMessageFeePayerSigner(
        session,
        appendTransactionMessageInstruction(
          instruction,
          createTransactionMessage({ version: 0 })
        )
      )
    )

    const signed = await signTransactionMessageWithSigners(message)
    // The wire bytes are base64, and this RPC method does not assume it: the
    // overload without `encoding` is the deprecated base58 one, so omitting it
    // makes the node decode base64 as base58 and reject the send with
    // `invalid base58 encoding` before the transaction is ever simulated.
    const signature = await client.rpc
      .sendTransaction(getBase64EncodedWireTransaction(signed), { encoding: 'base64' })
      .send()

    for (let i = 0; i < 90; i++) {
      const status = (await client.rpc.getSignatureStatuses([signature]).send()).value[0]
      if (status) {
        if (status.err) throw new Error(appT('errors.paymentRejected'))
        return signature
      }
      await new Promise((resolve) => {
        setTimeout(resolve, 1000)
      })
    }
    throw new Error(appT('errors.notConfirmed'))
  }

  /** Lamports a sweep of this session key would hand back to the buyer. */
  async function sweepableFloat(sessionAddress: Address): Promise<number> {
    const balance = Number(
      (await client.rpc.getBalance(sessionAddress, { commitment: 'confirmed' }).send()).value
    )
    return sweepableLamports(balance)
  }

  /**
   * Hands this device's session-key float back to the buyer. The session key
   * signs and is its own fee payer, so the sweep costs no wallet approval; the
   * program cannot do this itself because the key is not a PDA. Answers
   * `swept: 0` when there is nothing to send, and throws only when the float is
   * still there — the caller keeps the key in that case rather than leaving the
   * float on a key it can no longer sign with.
   */
  async function sweepSessionFloat(
    buyer: Address
  ): Promise<{ swept: number, signature: string | null }> {
    const session = await peekSessionSigner(buyer)
    if (!session) return { swept: 0, signature: null }
    const swept = await sweepableFloat(session.address)
    if (swept <= 0) return { swept: 0, signature: null }
    try {
      const signature = await submitWithSession(
        session,
        transferLamportsInstruction(session.address, buyer, swept)
      )
      return { swept, signature }
    } catch (error) {
      // A send can land and still time out in the confirmation poll below, and
      // reporting that as a failure would keep a key whose float has already
      // moved home — and show a retry for money that is back. The balance is
      // the truth; only a key that still holds a float failed to sweep.
      const left = await sweepableFloat(session.address).catch(() => swept)
      if (left > 0) throw error
      return { swept, signature: null }
    }
  }

  /** The token account a budget PDA owns for one mint: where its tokens sit. */
  async function tokenVault(budgetPda: Address, mint: Address): Promise<Address> {
    return await deriveAssociatedTokenAddress(budgetPda, mint)
  }

  /** `null` when the buyer has no allowance for this token yet. */
  async function loadTokenBudget(
    buyer: Address,
    mint: Address
  ): Promise<TokenBudgetState | null> {
    const pda = await deriveTokenBudgetPda(buyer, mint)
    const info = await client.rpc
      .getAccountInfo(pda, { commitment: 'confirmed', encoding: 'base64' })
      .send()
    if (!info.value || !Array.isArray(info.value.data)) return null

    const bytes = Uint8Array.from(atob(info.value.data[0]), c => c.charCodeAt(0))
    const expected = await accountDiscriminator('TokenBudget')
    if (!expected.every((byte, i) => bytes[i] === byte)) {
      throw new Error(appT('errors.notTokenBudgetAccount'))
    }

    // No vault yet simply means nobody has funded it for this token.
    const amount = await client.rpc
      .getTokenAccountBalance(await tokenVault(pda, mint), { commitment: 'confirmed' })
      .send()
      .then(balance => Number(balance.value.amount))
      .catch(() => 0)

    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
    return {
      pda,
      buyer: getAddressDecoder().decode(bytes.subarray(8, 40)),
      sessionKey: getAddressDecoder().decode(bytes.subarray(40, 72)),
      mint: getAddressDecoder().decode(bytes.subarray(72, 104)),
      monthlyLimit: Number(view.getBigUint64(104, true)),
      spent: Number(view.getBigUint64(112, true)),
      monthIndex: Number(view.getBigUint64(120, true)),
      lamports: Number(info.value.lamports),
      amount
    }
  }

  /** Tokens still spendable this month (a new month resets the counter). */
  function tokenRemainingThisMonth(budget: TokenBudgetState): number {
    const spent = budget.monthIndex === currentMonthIndex() ? budget.spent : 0
    return Math.max(0, budget.monthlyLimit - spent)
  }

  /** Why a token invoice cannot ride this allowance, or `null` when it can. */
  async function tokenBudgetObstacle(
    budget: TokenBudgetState,
    amount: number
  ): Promise<string | null> {
    const session = await peekSessionSigner(budget.buyer)
    if (!session || session.address !== budget.sessionKey) {
      return appT('errors.noSessionKey')
    }
    if (amount > tokenRemainingThisMonth(budget)) {
      return appT('errors.tokenLimitUsedUp')
    }
    if (budget.amount < amount) {
      return appT('errors.tokenNearlyEmpty')
    }
    return null
  }

  /**
   * The transfers that refill the token floats for the next payment: one per
   * account that is short, and none when both already cover it. The buyer signs
   * them — a session key cannot fund the float it spends — so this returns
   * instructions rather than sending anything.
   */
  async function tokenFloatTopUpInstructions(
    buyer: Address,
    mint: Address
  ): Promise<Instruction[]> {
    const budget = await loadTokenBudget(buyer, mint)
    if (!budget) return []
    const [invoice, receipt, account, budgetRent, session] = await Promise.all([
      rentMinimum(TOKEN_PAYMENT_SPACE),
      rentMinimum(PAYMENT_RECEIPT_SPACE),
      rentMinimum(TOKEN_ACCOUNT_SPACE),
      rentMinimum(TOKEN_BUDGET_SPACE),
      client.rpc.getBalance(budget.sessionKey, { commitment: 'confirmed' }).send()
    ])
    const topUp = tokenFloatTopUp(
      tokenFloatNeed({ invoice, receipt, account }, NETWORK_FEE_LAMPORTS),
      {
        budget: Math.max(0, budget.lamports - budgetRent),
        session: Number(session.value)
      }
    )
    const transfers: Instruction[] = []
    if (topUp.budget > 0) {
      transfers.push(transferLamportsInstruction(buyer, budget.pda, topUp.budget))
    }
    if (topUp.session > 0) {
      transfers.push(transferLamportsInstruction(buyer, budget.sessionKey, topUp.session))
    }
    return transfers
  }

  /** Settle a token invoice from the allowance, with no wallet popup. */
  async function sendSilentTokenPayment(
    buyer: Address,
    params: {
      payment: Address
      seller: Address
      reference: Address
      mint: Address
      amount: number
      memo: string
    }
  ): Promise<string> {
    const session = await peekSessionSigner(buyer)
    if (!session) {
      throw new Error(appT('errors.noSessionKeyHere'))
    }

    const budgetPda = await deriveTokenBudgetPda(buyer, params.mint)
    const instruction = buildInstruction(
      'pay_from_token_budget',
      { amount: params.amount, memo: params.memo },
      {
        session_key: session.address,
        budget: budgetPda,
        vault: await tokenVault(budgetPda, params.mint),
        mint: params.mint,
        seller: params.seller,
        seller_token_account: await deriveAssociatedTokenAddress(params.seller, params.mint),
        payment: params.payment,
        receipt: await derivePaymentReceiptPda(params.payment),
        reference: params.reference
      }
    )

    return await submitWithSession(session, instruction)
  }

  /**
   * What one `open_token_budget` signature costs on top of the tokens it
   * moves: the session-key float, the budget account's rent and its refund
   * float, and the network fee.
   */
  async function openTokenBudgetCost(
    buyer: Address,
    mint: Address,
    sessionAddress?: Address
  ): Promise<{ rent: number, feeFloat: number, refundFloat: number, fee: number, total: number }> {
    const existing = await loadTokenBudget(buyer, mint)
    const rent = existing ? 0 : await rentMinimum(TOKEN_BUDGET_SPACE)
    const refundFloat = existing ? 0 : TOKEN_BUDGET_FEE_FLOAT
    const sessionLamports = sessionAddress
      ? Number(
          (await client.rpc.getBalance(sessionAddress, { commitment: 'confirmed' }).send()).value
        )
      : 0
    const feeFloat = Math.max(0, TOKEN_SESSION_FEE_FLOAT - sessionLamports)
    const fee = NETWORK_FEE_LAMPORTS
    return { rent, feeFloat, refundFloat, fee, total: rent + refundFloat + feeFloat + fee }
  }

  return {
    loadBudget,
    remainingThisMonth,
    availablePot,
    openBudgetCost,
    preflight,
    silentPaymentObstacle,
    sendSilentPayment,
    sweepableFloat,
    sweepSessionFloat,
    tokenVault,
    loadTokenBudget,
    tokenRemainingThisMonth,
    tokenBudgetObstacle,
    sendSilentTokenPayment,
    tokenFloatTopUpInstructions,
    openTokenBudgetCost
  }
}
