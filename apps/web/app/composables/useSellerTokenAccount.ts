import {
  AccountRole,
  address,
  type Address,
  type Instruction
} from '@vue-solana/nuxt/kit'
import type { SupportedCurrency } from './usePaymentApi'
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  compileWireTransaction,
  deriveAssociatedTokenAddress,
  mintForCurrency
} from './usePaymentProgram'

const SYSTEM_PROGRAM_ID = address('11111111111111111111111111111111')

/**
 * `CreateIdempotent` on the associated-token program: opens the owner's own
 * account for a mint, paid by the owner. The settle only ever pays into an
 * account that exists — creating one inside a payment would hand its rent to
 * whoever named the seller — so the seller opens theirs when they price in a
 * token.
 */
export function openTokenAccountInstruction(
  owner: Address,
  mint: Address,
  tokenAccount: Address
): Instruction {
  return {
    programAddress: ASSOCIATED_TOKEN_PROGRAM_ID,
    accounts: [
      // The rent payer and the owner are the same wallet: a seller's own account.
      { address: owner, role: AccountRole.WRITABLE_SIGNER },
      { address: tokenAccount, role: AccountRole.WRITABLE },
      { address: owner, role: AccountRole.READONLY },
      { address: mint, role: AccountRole.READONLY },
      { address: SYSTEM_PROGRAM_ID, role: AccountRole.READONLY },
      { address: TOKEN_PROGRAM_ID, role: AccountRole.READONLY }
    ],
    // Instruction 1 of the associated-token program.
    data: new Uint8Array([1])
  }
}

/**
 * The seller's side of a token invoice: their account for the price's mint,
 * opened in its own transaction. A SOL price, or an account that is already
 * there, asks for no signature at all.
 */
export function useSellerTokenAccount() {
  const { client } = useSolanaClient()
  const { execute } = useSolanaSignAndSendTransaction()

  async function ensureSellerTokenAccount(
    seller: Address,
    currency: SupportedCurrency,
    cluster: string
  ): Promise<void> {
    if (currency === 'sol') return
    const mint = mintForCurrency(currency, cluster)
    // A cluster with no mint for this currency has nothing to open.
    if (!mint) return
    const tokenAccount = await deriveAssociatedTokenAddress(seller, mint)

    const existing = await client.rpc
      .getAccountInfo(tokenAccount, { commitment: 'confirmed', encoding: 'base64' })
      .send()
    if (existing.value) return

    const wire = await compileWireTransaction({
      instruction: openTokenAccountInstruction(seller, mint, tokenAccount),
      payer: seller,
      rpc: client.rpc
    })
    await execute(wire, {
      confirm: true,
      confirmation: { commitment: 'confirmed', timeoutMs: 120_000 }
    })
  }

  return { ensureSellerTokenAccount }
}
