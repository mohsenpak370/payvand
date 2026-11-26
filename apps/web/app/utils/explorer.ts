/**
 * Where a buyer can check a signature for themselves.
 *
 * The explorer needs the cluster the payment landed on: a devnet signature on
 * the mainnet explorer is a 404, which reads to a buyer as "the payment is not
 * there". `localnet` has no public explorer at all, so it gets no link rather
 * than one that cannot resolve.
 */
export function explorerTxUrl(signature: string, cluster: string): string | null {
  if (!signature) return null
  const url = `https://explorer.solana.com/tx/${signature}`
  if (cluster === 'devnet' || cluster === 'testnet') return `${url}?cluster=${cluster}`
  if (cluster === 'mainnet' || cluster === 'mainnet-beta') return url
  return null
}

/** The explorer page for an account, on the cluster it lives on. */
export function explorerAddressUrl(address: string, cluster: string): string | null {
  if (!address) return null
  const url = `https://explorer.solana.com/address/${address}`
  if (cluster === 'devnet' || cluster === 'testnet') return `${url}?cluster=${cluster}`
  if (cluster === 'mainnet' || cluster === 'mainnet-beta') return url
  return null
}
