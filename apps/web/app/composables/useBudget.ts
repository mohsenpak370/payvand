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
  type Address,
  type KeyPairSigner
} from '@vue-solana/nuxt/kit'
import { PAYVAND_PROGRAM_ID, buildInstruction } from './usePaymentProgram'

const BUDGET_SEED = new TextEncoder().encode('budget')
const SESSION_STORAGE_PREFIX = 'payvand.session.v1.'

/** Mirrors `BUDGET_SPACE` in the program: 8 + 32 + 32 + 8 + 8 + 8 + 1. */
const BUDGET_SPACE = 97
/** Mirrors `PAYMENT_SPACE` in the program: 8 + 32 + 32 + 8 + 4 + 96 + 1 + 32 + 8 + 8 + 1. */
const PAYMENT_SPACE = 230

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

  return {
    loadBudget,
    remainingThisMonth,
    availablePot,
    silentPaymentObstacle,
    sendSilentPayment
  }
}
