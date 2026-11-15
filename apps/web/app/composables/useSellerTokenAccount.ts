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
 * account for a mint, paid by the owner, and succeeds without doing anything
 * when the account is already there. The settle only ever pays into an account
 * that exists — creating one inside a payment would hand its rent to whoever
 * named the seller — so the seller opens theirs when they price in a token.
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
 * The seller's side of a token invoice: the accounts the currencies need, in
 * their own transaction. Nothing is created when they already exist, so
 * re-pricing in the same token costs no signature.
 */
export function useSellerTokenAccount() {
  const { client } = useSolanaClient()
  const { execute } = useSolanaSignAndSendTransaction()

  async function ensureSellerTokenAccounts(
    seller: Address,
    currencies: SupportedCurrency[],
    cluster: string
  ): Promise<number> {
    const mints = [...new Set(currencies
      .map(currency => (currency === 'sol' ? null : mintForCurrency(currency, cluster)))
      .filter((mint): mint is Address => Boolean(mint)))]
    if (!mints.length) return 0

    const missing: Instruction[] = []
    for (const mint of mints) {
      const tokenAccount = await deriveAssociatedTokenAddress(seller, mint)
      const info = await client.rpc
        .getAccountInfo(tokenAccount, { commitment: 'confirmed', encoding: 'base64' })
        .send()
      if (!info.value) missing.push(openTokenAccountInstruction(seller, mint, tokenAccount))
    }
    if (!missing.length) return 0

    const wire = await compileWireTransaction({
      instruction: missing,
      payer: seller,
      rpc: client.rpc
    })
    await execute(wire, {
      confirm: true,
      confirmation: { commitment: 'confirmed', timeoutMs: 120_000 }
    })
    return missing.length
  }

  return { ensureSellerTokenAccounts }
}
