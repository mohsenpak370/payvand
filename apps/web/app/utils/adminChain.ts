import type { ChainAccount, ChainOverview } from '~/composables/useAdminApi'

/** USDC and EURC both have six decimals. */
export function tokenUnits(baseUnits: number): string {
  return (baseUnits / 1_000_000).toFixed(6).replace(/\.?0+$/, '')
}

/**
 * Lamports above the rent-exempt minimum: what the account could spend (or a
 * program-owned one could pay out) without dropping below it. Never negative.
 */
export function rentSurplus(account: Pick<ChainAccount, 'lamports' | 'rent_exempt_lamports'>): number {
  return Math.max(0, account.lamports - account.rent_exempt_lamports)
}

/**
 * How many more starter-token claims the faucet can pay for: the scarcest of
 * its SOL and its two stablecoins, against what one claim sends. A missing
 * token account is zero of that token, and so zero claims.
 */
export function claimsLeft(overview: ChainOverview): number | null {
  const faucet = overview.wallets.find(wallet => wallet.key === 'faucet')?.account
  if (!faucet || !faucet.exists) return faucet ? 0 : null
  const perSol = overview.faucet_sol_lamports
  const perToken = overview.faucet_token_base_units
  if (perSol <= 0 || perToken <= 0) return null
  return Math.floor(Math.min(
    faucet.lamports / perSol,
    (faucet.tokens?.usdc_base_units ?? 0) / perToken,
    (faucet.tokens?.eurc_base_units ?? 0) / perToken
  ))
}

/** How many sponsored settles the relayer can front from its SOL right now. */
export function settlesLeft(overview: ChainOverview): number | null {
  const sponsor = overview.wallets.find(wallet => wallet.key === 'sponsor')?.account
  if (!sponsor) return null
  if (overview.sponsor_settle_lamports <= 0) return null
  return Math.floor(sponsor.lamports / overview.sponsor_settle_lamports)
}

/** Below these the page flags a wallet as running low. */
export const LOW_CLAIMS = 10
export const LOW_SETTLES = 50
