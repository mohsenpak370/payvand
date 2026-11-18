import { getAddressDecoder, type Address } from '@vue-solana/nuxt/kit'
import { accountDiscriminator, deriveAssociatedTokenAddress } from './usePaymentProgram'
import { deriveBudgetPda, deriveTokenBudgetPda, type BudgetState, type TokenBudgetState } from './budgetMath'

/**
 * How long a budget account read stays fresh. A currency toggle or a repeated
 * quote asks for the same account again within a click, and nothing on chain can
 * have changed in between; a minute keeps those reads cheap without hiding a
 * real change for long.
 */
const BUDGET_CACHE_TTL_MS = 60_000

const budgetAccountCache = new Map<string, { readAt: number, state: BudgetState | null }>()
const tokenBudgetAccountCache = new Map<
  string,
  { readAt: number, state: TokenBudgetState | null }
>()

/** Drops cached budget and vault reads, so the next load asks the cluster again. */
export function clearBudgetAccountCache(): void {
  budgetAccountCache.clear()
  tokenBudgetAccountCache.clear()
}

/** The token account a budget PDA owns for one mint: where its tokens sit. */
export async function tokenVault(budgetPda: Address, mint: Address): Promise<Address> {
  return await deriveAssociatedTokenAddress(budgetPda, mint)
}

/** `null` when the buyer has no budget yet. */
export async function loadBudgetAccount(
  client: ReturnType<typeof useSolanaClient>['client'],
  buyer: Address,
  options: { force?: boolean } = {}
): Promise<BudgetState | null> {
  const key = buyer as string
  const cached = budgetAccountCache.get(key)
  if (!options.force && cached && Date.now() - cached.readAt < BUDGET_CACHE_TTL_MS) {
    return cached.state
  }

  const pda = await deriveBudgetPda(buyer)
  const info = await client.rpc
    .getAccountInfo(pda, { commitment: 'confirmed', encoding: 'base64' })
    .send()
  let state: BudgetState | null = null
  if (info.value && Array.isArray(info.value.data)) {
    const bytes = Uint8Array.from(atob(info.value.data[0]), (c: string) => c.charCodeAt(0))
    const expected = await accountDiscriminator('Budget')
    if (!expected.every((byte, i) => bytes[i] === byte)) {
      throw new Error(appT('errors.notBudgetAccount'))
    }

    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
    state = {
      pda,
      buyer: getAddressDecoder().decode(bytes.subarray(8, 40)),
      sessionKey: getAddressDecoder().decode(bytes.subarray(40, 72)),
      monthlyLimit: Number(view.getBigUint64(72, true)),
      spent: Number(view.getBigUint64(80, true)),
      monthIndex: Number(view.getBigUint64(88, true)),
      lamports: Number(info.value.lamports)
    }
  }

  budgetAccountCache.set(key, { readAt: Date.now(), state })
  return state
}

/** `null` when the buyer has no allowance for this token yet. */
export async function loadTokenBudgetAccount(
  client: ReturnType<typeof useSolanaClient>['client'],
  buyer: Address,
  mint: Address,
  options: { force?: boolean } = {}
): Promise<TokenBudgetState | null> {
  const key = `${buyer as string}:${mint as string}`
  const cached = tokenBudgetAccountCache.get(key)
  if (!options.force && cached && Date.now() - cached.readAt < BUDGET_CACHE_TTL_MS) {
    return cached.state
  }

  const pda = await deriveTokenBudgetPda(buyer, mint)
  const info = await client.rpc
    .getAccountInfo(pda, { commitment: 'confirmed', encoding: 'base64' })
    .send()
  let state: TokenBudgetState | null = null
  if (info.value && Array.isArray(info.value.data)) {
    const bytes = Uint8Array.from(atob(info.value.data[0]), (c: string) => c.charCodeAt(0))
    const expected = await accountDiscriminator('TokenBudget')
    if (!expected.every((byte, i) => bytes[i] === byte)) {
      throw new Error(appT('errors.notTokenBudgetAccount'))
    }

    // No vault yet simply means nobody has funded it for this token.
    const amount = await client.rpc
      .getTokenAccountBalance(await tokenVault(pda, mint), { commitment: 'confirmed' })
      .send()
      .then((balance: { value: { amount: string } }) => Number(balance.value.amount))
      .catch(() => 0)

    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
    state = {
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

  tokenBudgetAccountCache.set(key, { readAt: Date.now(), state })
  return state
}
