import { describe, expect, it } from 'vitest'
import { explorerTxUrl } from './explorer'

/**
 * A link that opens the wrong cluster — or nothing — is worse than no link: the
 * buyer reads a 404 as "the payment never happened", when all that happened is
 * that the explorer was asked about the wrong network.
 */
const SIGNATURE = '5J8xRn7NvYhkhCLZ8HHVwU5CTK8rQuTgYFiKn9bJvZ8HwGaP3wWiHjFx4dwm1x3Kk4vKcBXKFuXsRXS1QqTqC3o'

describe('explorerTxUrl', () => {
  it('names the cluster for a devnet or testnet payment', () => {
    expect(explorerTxUrl(SIGNATURE, 'devnet'))
      .toBe(`https://explorer.solana.com/tx/${SIGNATURE}?cluster=devnet`)
    expect(explorerTxUrl(SIGNATURE, 'testnet'))
      .toBe(`https://explorer.solana.com/tx/${SIGNATURE}?cluster=testnet`)
  })

  it('leaves the query off mainnet, where the explorer expects none', () => {
    expect(explorerTxUrl(SIGNATURE, 'mainnet')).toBe(`https://explorer.solana.com/tx/${SIGNATURE}`)
    expect(explorerTxUrl(SIGNATURE, 'mainnet-beta')).toBe(`https://explorer.solana.com/tx/${SIGNATURE}`)
  })

  it('returns no link where there is nothing to open', () => {
    // Localnet has no public explorer, and an unknown cluster has no known one.
    expect(explorerTxUrl(SIGNATURE, 'localnet')).toBeNull()
    expect(explorerTxUrl(SIGNATURE, 'somewhere-else')).toBeNull()
  })

  it('returns no link for a signature it does not have', () => {
    expect(explorerTxUrl('', 'devnet')).toBeNull()
  })
})
