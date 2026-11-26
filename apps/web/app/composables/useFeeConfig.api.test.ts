import { address } from '@vue-solana/nuxt/kit'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  clearFeeConfigCache,
  clearInvoiceAccountCache,
  clearInvoiceRentCache,
  useInvoiceCosts
} from './useFeeConfig'
import { accountDiscriminator, deriveConfigPda } from './usePaymentProgram'

/**
 * A quote prices from `GET /api/config`, not from the cluster.
 *
 * The policy, the mints and the rent minimums all live on chain, and every tab
 * used to read them itself: an account read plus a rent read per size, per
 * minute, per browser. The API answers them once a minute for everybody, so the
 * quote here must not touch the cluster at all — and when the API cannot answer,
 * it must fall back to reading the account rather than refuse to quote.
 */

const mint = address('4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU')
const admin = address('6dQgFZDTkhDMTZv9ftZe4ufU9jG5fvJEw6zpw6WCKjPz')

let accountReads: string[] = []
let rentReads = 0
let getConfig: ReturnType<typeof vi.fn>
let quote: ReturnType<typeof useInvoiceCosts>

/** The answer `/api/config` gives: a 2% policy and the cluster's two rents. */
function apiConfig() {
  return {
    cluster: 'devnet',
    program_id: 'payvand',
    admin,
    fee_mode: 'seller' as const,
    session_fee_float: 12_000_000,
    fees: {
      fee_bps_below: 200,
      fee_bps_above: 200,
      sol_tier_lamports: 0,
      token_tier_base_units: 0,
      flat_usdc_base_units: 0,
      flat_eurc_base_units: 0,
      network_fee_lamports: 5_000,
      network_fee_usdc_base_units: 351,
      network_fee_eurc_base_units: 320
    },
    mints: { usdc: mint, eurc: mint },
    spaces: {
      payment: 230,
      token_payment: 262,
      payment_receipt: 48,
      token_account: 165,
      budget: 97,
      token_budget: 129
    },
    rent: {
      payment: 230_000,
      token_payment: 262_000,
      payment_receipt: 48_000,
      token_account: 165_000,
      budget: 97_000,
      token_budget: 129_000
    }
  }
}

/** A `Config` account with a 1% policy, for the fallback path. */
async function configAccount(): Promise<string> {
  const bytes = new Uint8Array(184)
  bytes.set(await accountDiscriminator('Config'), 0)
  const view = new DataView(bytes.buffer)
  view.setUint16(40, 100, true)
  view.setUint16(42, 100, true)
  const usdc = Uint8Array.from(base58(mint))
  bytes.set(usdc, 76)
  bytes.set(usdc, 108)
  bytes[164] = 0
  view.setBigUint64(165, 12_000_000n, true)
  bytes[181] = 255
  bytes[182] = 254
  bytes[183] = 4
  return Buffer.from(bytes).toString('base64')
}

/** The 32 bytes behind a base58 address, which `Config` stores raw. */
function base58(value: string): number[] {
  const alphabet = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
  let num = 0n
  for (const char of value) num = num * 58n + BigInt(alphabet.indexOf(char))
  const bytes = new Array<number>(32).fill(0)
  for (let i = 31; i >= 0 && num > 0n; i--) {
    bytes[i] = Number(num & 0xffn)
    num >>= 8n
  }
  return bytes
}

beforeEach(() => {
  clearFeeConfigCache()
  clearInvoiceAccountCache()
  clearInvoiceRentCache()
  accountReads = []
  rentReads = 0

  vi.stubGlobal('appT', (key: string) => key)
  vi.stubGlobal('useSolanaClient', () => ({
    client: {
      rpc: {
        getAccountInfo: (account: string) => ({
          send: async () => {
            accountReads.push(account)
            return { value: { data: [await configAccount()] } }
          }
        }),
        getMinimumBalanceForRentExemption: (space: bigint) => ({
          send: async () => {
            rentReads++
            return Number(space) * 2_000
          }
        })
      }
    }
  }))

  getConfig = vi.fn(async () => apiConfig())
  vi.stubGlobal('usePaymentApi', () => ({ getConfig }))
  quote = useInvoiceCosts()
})

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

describe('a quote priced from /api/config', () => {
  it('reads the policy and the rents from the API, not from the cluster', async () => {
    const sol = await quote.estimateInvoiceCosts(10_000_000, 'sol', { cluster: 'devnet' })

    expect(getConfig).toHaveBeenCalledTimes(1)
    // The invoice's cluster travels with the question: the API answers for the
    // cluster the checkout is pointed at, not its own default.
    expect(getConfig).toHaveBeenCalledWith('devnet')
    // The API's 2% policy, and its rents rather than the stub's per-byte ones.
    expect(sol.fee).toBe(152_000)
    expect(sol.invoiceRent).toBe(230_000)
    expect(sol.receiptRent).toBe(48_000)
    expect(accountReads).toEqual([])
    expect(rentReads).toBe(0)
  })

  it('answers the next quote inside the minute without asking the API again', async () => {
    await quote.estimateInvoiceCosts(10_000_000, 'sol', { cluster: 'devnet' })
    const second = await useInvoiceCosts()
      .estimateInvoiceCosts(10_000_000, 'sol', { cluster: 'devnet' })

    expect(getConfig).toHaveBeenCalledTimes(1)
    expect(second.invoiceRent).toBe(230_000)
    expect(rentReads).toBe(0)
  })

  it('asks for another cluster instead of reusing the cached policy', async () => {
    await quote.estimateInvoiceCosts(10_000_000, 'sol', { cluster: 'devnet' })
    getConfig.mockResolvedValue({ ...apiConfig(), cluster: 'mainnet-beta' })

    const mainnet = await quote.estimateInvoiceCosts(10_000_000, 'sol', {
      cluster: 'mainnet-beta'
    })

    expect(getConfig).toHaveBeenLastCalledWith('mainnet-beta')
    // Priced again, not answered from devnet's minute-old policy.
    expect(mainnet.invoiceRent).toBe(230_000)
    expect(getConfig).toHaveBeenCalledTimes(2)
  })

  it('keeps the API policy but reads the rents itself when they come back zero', async () => {
    getConfig.mockResolvedValue({
      ...apiConfig(),
      rent: Object.fromEntries(Object.keys(apiConfig().rent).map(name => [name, 0]))
    })

    const sol = await quote.estimateInvoiceCosts(10_000_000, 'sol', { cluster: 'devnet' })

    // The 2% policy is still the API's, so the fee is not the fallback's 1%:
    // 200,000 of fee, less the 96,000 of marker rent the chain's rent implies.
    expect(sol.fee).toBe(104_000)
    // A zero rent is not an answer: the quote asks the cluster instead.
    expect(sol.invoiceRent).toBe(460_000)
    expect(rentReads).toBe(2)
  })

  it('falls back to the account when the API cannot answer', async () => {
    getConfig.mockRejectedValue(new Error('503'))

    const sol = await quote.estimateInvoiceCosts(10_000_000, 'sol', { cluster: 'devnet' })

    // The chain's own 1% policy and the stub's 2,000 lamports per byte: 1% of
    // 10,000,000 is 100,000, less the 96,000 marker rent the seller pays out of
    // the price directly.
    expect(sol.fee).toBe(4_000)
    expect(sol.invoiceRent).toBe(460_000)
    expect(accountReads).toEqual([await deriveConfigPda()])
  })

  it('falls back when the API answers a policy a quote cannot use', async () => {
    getConfig.mockResolvedValue({ ...apiConfig(), mints: { usdc: '', eurc: '' } })

    const sol = await quote.estimateInvoiceCosts(10_000_000, 'sol', { cluster: 'devnet' })

    expect(sol.fee).toBe(4_000)
    expect(accountReads).toHaveLength(1)
  })
})
