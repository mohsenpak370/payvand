import { address } from '@vue-solana/nuxt/kit'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { clearFeeConfigCache, clearInvoiceAccountCache, useInvoiceCosts } from './useFeeConfig'
import {
  accountDiscriminator,
  deriveAssociatedTokenAddress,
  deriveConfigPda,
  deriveTreasuryTokenAccount
} from './usePaymentProgram'

/**
 * A token quote asks the cluster two questions besides the fee policy: does the
 * seller already have an account for the mint, and does the treasury. Switching
 * currency and back asks them again within a click, and neither account can have
 * appeared in between, so they are held for the fee policy's minute.
 *
 * `useSolanaClient` and `appT` are the only context this composable wants, so
 * the cluster is a fake that records what it was asked — which is the whole
 * point: a repeat quote must add nothing to that list.
 */

const seller = address('6dQgFZDTkhDMTZv9ftZe4ufU9jG5fvJEw6zpw6WCKjPz')
const mint = address('4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU')

/** `CONFIG_SIZE` from `useFeeConfig.ts`: the account the quote will decode. */
const CONFIG_SIZE = 62

let asked: string[] = []
let rentReads = 0
let held: Set<string>
let configPda: string
let sellerAta: string
let treasuryAta: string
let quote: ReturnType<typeof useInvoiceCosts>

/** A `Config` account with a 2% seller-paid fee, as the program writes one. */
async function configAccount(): Promise<string> {
  const bytes = new Uint8Array(CONFIG_SIZE)
  bytes.set(await accountDiscriminator('Config'), 0)
  const view = new DataView(bytes.buffer)
  view.setUint16(40, 200, true)
  view.setBigUint64(42, 0n, true)
  bytes[50] = 0
  view.setBigUint64(51, 12_000_000n, true)
  return Buffer.from(bytes).toString('base64')
}

beforeEach(async () => {
  clearFeeConfigCache()
  clearInvoiceAccountCache()
  asked = []
  rentReads = 0

  const config = await configAccount()
  configPda = await deriveConfigPda()
  sellerAta = await deriveAssociatedTokenAddress(seller, mint)
  treasuryAta = await deriveTreasuryTokenAccount(mint)
  // The policy and the seller's account are on chain; the treasury has not
  // opened an account for this mint yet, which is the state the quote reports.
  held = new Set([configPda, sellerAta])

  vi.stubGlobal('appT', (key: string) => key)
  vi.stubGlobal('useSolanaClient', () => ({
    client: {
      rpc: {
        getAccountInfo: (account: string) => ({
          send: async () => {
            asked.push(account)
            return { value: held.has(account) ? { data: [account === configPda ? config : ''] } : null }
          }
        }),
        getMinimumBalanceForRentExemption: (space: bigint) => ({
          send: async () => {
            rentReads++
            return Number(space) * 1_000
          }
        })
      }
    }
  }))

  quote = useInvoiceCosts()
})

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

const usdcQuote = () => quote.estimateInvoiceCosts(1_500_000, 'usdc', { mint, seller })

describe('estimateInvoiceCosts account reads', () => {
  it('quotes the same invoice again without asking the cluster anything', async () => {
    const first = await usdcQuote()

    // The policy, then the two accounts: one read each, and the two rents.
    expect(asked).toEqual([configPda, sellerAta, treasuryAta])
    expect(rentReads).toBe(2)
    expect(first.treasuryTokenAccountMissing).toBe(true)
    expect(first.sellerTokenAccountRent).toBe(0)

    await usdcQuote()

    expect(asked).toHaveLength(3)
    expect(rentReads).toBe(2)
  })

  it('asks nothing new when the buyer toggles currency and back', async () => {
    await usdcQuote()
    const afterUsdc = asked.length

    // A SOL price reads no token account at all, and the policy is cached.
    await quote.estimateInvoiceCosts(1_500_000, 'sol')
    expect(asked).toHaveLength(afterUsdc)

    await usdcQuote()
    expect(asked).toHaveLength(afterUsdc)
  })

  it('asks again once the minute is up, so a new account is noticed', async () => {
    vi.useFakeTimers({ now: 1_760_000_000_000 })
    await usdcQuote()
    expect(asked).toHaveLength(3)

    vi.setSystemTime(Date.now() + 61_000)
    await usdcQuote()

    // The policy, the seller's account and the treasury's, all re-read — the
    // cache is a minute's answer, not a permanent one.
    expect(asked).toEqual([configPda, sellerAta, treasuryAta, configPda, sellerAta, treasuryAta])
  })
})
