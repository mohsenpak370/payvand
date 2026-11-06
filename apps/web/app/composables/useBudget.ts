import {
  appendTransactionMessageInstruction,
  createKeyPairSignerFromPrivateKeyBytes,
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
  type KeyPairSigner
} from '@vue-solana/nuxt/kit'
import {
  PAYVAND_PROGRAM_ID,
  buildInstruction,
  deriveAssociatedTokenAddress,
  describeProgramErrorCode
} from './usePaymentProgram'
import type { compileWireTransaction } from './usePaymentProgram'

const BUDGET_SEED = new TextEncoder().encode('budget')
/** A token budget per buyer per mint, under its own seed prefix. */
const TOKEN_BUDGET_SEED = new TextEncoder().encode('token-budget')
const SESSION_STORAGE_PREFIX = 'payvand.session.v1.'

/** Mirrors `BUDGET_SPACE` in the program: 8 + 32 + 32 + 8 + 8 + 8 + 1. */
const BUDGET_SPACE = 97
/** Mirrors `PAYMENT_SPACE` in the program: 8 + 32 + 32 + 8 + 4 + 96 + 1 + 32 + 8 + 8 + 1. */
const PAYMENT_SPACE = 230
/**
 * Mirrors `SESSION_FEE_FLOAT` in the program — lamports parked on the
 * session key so its silent transactions can pay their own fees.
 */
const SESSION_FEE_FLOAT = 10_000_000
/** Mirrors `TOKEN_BUDGET_SPACE`: 8 + 32 + 32 + 32 + 8 + 8 + 8 + 1. */
const TOKEN_BUDGET_SPACE = 129
/** Lamports a token budget keeps to refund the invoice rent it advances. */
const TOKEN_BUDGET_FEE_FLOAT = 10_000_000
/**
 * Network fee for one signature (5000 lamports), plus a small margin for
 * price movement — what `open_budget` costs above the lamports it moves.
 */
const TRANSACTION_FEE_LAMPORTS = 10_000

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

const sessionStorageKey = (buyer: Address) => `${SESSION_STORAGE_PREFIX}${buyer}`

/** The device's stored session key, or null when this device never opened one. */
export async function peekSessionSigner(buyer: Address): Promise<KeyPairSigner | null> {
  try {
    const raw = localStorage.getItem(sessionStorageKey(buyer))
    if (!raw) return null
    const bytes = Uint8Array.from(JSON.parse(raw) as number[])
    if (bytes.length !== 32) return null
    return await createKeyPairSignerFromPrivateKeyBytes(bytes)
  } catch {
    return null
  }
}

/**
 * The session key for this buyer: loaded when this device has one, freshly
 * generated and persisted otherwise. The raw key never leaves the device —
 * and a leaked key can only ever spend the remainder of the month's budget.
 */
export async function ensureSessionSigner(buyer: Address): Promise<KeyPairSigner> {
  const existing = await peekSessionSigner(buyer)
  if (existing) return existing
  const bytes = crypto.getRandomValues(new Uint8Array(32))
  localStorage.setItem(sessionStorageKey(buyer), JSON.stringify([...bytes]))
  return await createKeyPairSignerFromPrivateKeyBytes(bytes)
}

/** Drop the device's session key — used after the budget is closed. */
export function forgetSessionSigner(buyer: Address): void {
  localStorage.removeItem(sessionStorageKey(buyer))
}

/** First 8 bytes of `sha256("account:<Name>")`, Anchor's account tag. */
async function accountDiscriminator(name: string): Promise<Uint8Array> {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(`account:${name}`)
  )
  return new Uint8Array(digest.slice(0, 8))
}

/** Reads and interprets a buyer's budget straight from the cluster. */
export function useBudget() {
  const { client } = useSolanaClient()

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
      throw new Error('That address is not a budget account.')
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
   * the pot top-up, the session key's fee float, the budget account's rent
   * (only when it does not exist yet) and the transaction fee. Mirrors the
   * transfers the program performs, so the UI can warn *before* the wallet
   * is asked to sign something that would fail pre-flight.
   */
  async function openBudgetCost(
    buyer: Address,
    monthlyLimit: number,
    sessionAddress: Address
  ): Promise<{ topUp: number, feeFloat: number, rent: number, fee: number, total: number }> {
    const existing = await loadBudget(buyer)
    const rent = existing ? 0 : await rentMinimum(BUDGET_SPACE)
    const pot = existing ? await availablePot(existing) : 0
    const topUp = Math.max(0, monthlyLimit - pot)
    const sessionLamports = Number(
      (await client.rpc.getBalance(sessionAddress, { commitment: 'confirmed' }).send()).value
    )
    const feeFloat = Math.max(0, SESSION_FEE_FLOAT - sessionLamports)
    const fee = TRANSACTION_FEE_LAMPORTS
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
      throw new Error(
        'This wallet does not hold enough SOL for that limit. Lower it or top the wallet up first.'
      )
    }
    throw new Error(
      'The transaction would be rejected on-chain, so it was not sent. '
      + 'If you have no budget yet, the limit must be greater than 0; '
      + 'otherwise reload the page and try again.'
    )
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
      return 'This device has no session key for your budget — open it once on this device.'
    }
    if (amount > remainingThisMonth(budget)) {
      return 'The monthly limit is used up for this month.'
    }
    const needed = amount + (await rentMinimum(PAYMENT_SPACE))
    if ((await availablePot(budget)) < needed) {
      return 'The budget is nearly empty — top it up from the budget page.'
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
      throw new Error('No session key on this device — open your budget here first.')
    }

    const instruction = buildInstruction(
      'pay_from_budget',
      { amount: params.amount, memo: params.memo },
      {
        session_key: session.address,
        budget: await deriveBudgetPda(buyer),
        payment: params.payment,
        seller: params.seller,
        reference: params.reference
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
    instruction: ReturnType<typeof buildInstruction>
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
    const signature = await client.rpc
      .sendTransaction(getBase64EncodedWireTransaction(signed))
      .send()

    for (let i = 0; i < 90; i++) {
      const status = (await client.rpc.getSignatureStatuses([signature]).send()).value[0]
      if (status) {
        if (status.err) throw new Error('The payment was rejected on-chain.')
        return signature
      }
      await new Promise((resolve) => {
        setTimeout(resolve, 1000)
      })
    }
    throw new Error('The payment was sent but did not confirm in time — watch the invoice status.')
  }

  // ---- token allowances ---------------------------------------------------

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
      throw new Error('That address is not a token budget account.')
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
      return 'This device has no session key for your budget — open it once on this device.'
    }
    if (amount > tokenRemainingThisMonth(budget)) {
      return 'The monthly limit for this token is used up.'
    }
    if (budget.amount < amount) {
      return 'The allowance does not hold enough of this token — top it up from the budget page.'
    }
    return null
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
      throw new Error('No session key on this device — open your budget here first.')
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
    sessionAddress: Address
  ): Promise<{ rent: number, feeFloat: number, refundFloat: number, fee: number, total: number }> {
    const existing = await loadTokenBudget(buyer, mint)
    const rent = existing ? 0 : await rentMinimum(TOKEN_BUDGET_SPACE)
    const refundFloat = existing ? 0 : TOKEN_BUDGET_FEE_FLOAT
    const sessionLamports = Number(
      (await client.rpc.getBalance(sessionAddress, { commitment: 'confirmed' }).send()).value
    )
    const feeFloat = Math.max(0, SESSION_FEE_FLOAT - sessionLamports)
    const fee = TRANSACTION_FEE_LAMPORTS
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
    tokenVault,
    loadTokenBudget,
    tokenRemainingThisMonth,
    tokenBudgetObstacle,
    sendSilentTokenPayment,
    openTokenBudgetCost
  }
}
