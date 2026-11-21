import { address } from '@vue-solana/nuxt/kit'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  clearFeeConfigCache,
  clearInvoiceAccountCache,
  clearInvoiceRentCache,
  useInvoiceCosts
} from './useFeeConfig'
import {
  accountDiscriminator,
  deriveAssociatedTokenAddress,
  deriveConfigPda,
  deriveTreasuryTokenAccount
} from './usePaymentProgram'
import { feeOn } from '../utils/invoiceCost'

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
const CONFIG_SIZE = 152

let asked: string[] = []
let rentReads = 0
let held: Set<string>
let configPda: string
let sellerAta: string
let treasuryAta: string
let quote: ReturnType<typeof useInvoiceCosts>

/**
 * A `Config` account with a 2% seller-paid policy, as the program writes one:
 * a 0 boundary (so every amount pays the one rate) and no named currencies, so
 * no invoice in this suite pays a flat.
 */
async function configAccount(): Promise<string> {
  const bytes = new Uint8Array(CONFIG_SIZE)
  bytes.set(await accountDiscriminator('Config'), 0)
  const view = new DataView(bytes.buffer)
  view.setUint16(40, 200, true) // fee_bps_below
  view.setUint16(42, 200, true) // fee_bps_above
  view.setBigUint64(44, 0n, true) // sol_tier_lamports
  view.setBigUint64(52, 0n, true) // token_tier_base_units
  view.setBigUint64(60, 0n, true) // flat_usdc_base_units
  view.setBigUint64(68, 0n, true) // flat_eurc_base_units
  bytes[140] = 0 // fee_mode: SellerPays
  view.setBigUint64(141, 12_000_000n, true)
  bytes[149] = 255 // bump
  bytes[150] = 254 // treasury_bump
  bytes[151] = 2 // version
  return Buffer.from(bytes).toString('base64')
}

beforeEach(async () => {
  clearFeeConfigCache()
  clearInvoiceAccountCache()
  clearInvoiceRentCache()
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

  it('hands a second composable instance the rents the first one read', async () => {
    await usdcQuote()
    expect(rentReads).toBe(2)

    // Another page's `useInvoiceCosts`: the same invoice, quoted from the same
    // module-level caches, so neither the rents nor the two accounts are asked
    // for again.
    const second = useInvoiceCosts()
    const again = await second.estimateInvoiceCosts(1_500_000, 'usdc', { mint, seller })

    expect(rentReads).toBe(2)
    expect(asked).toHaveLength(3)
    expect(again).toEqual(await usdcQuote())
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

/**
 * The quote a screen renders is the program's arithmetic, not a nearby copy of
 * it: `fee_for` (`helpers.rs`) floors `amount * fee_bps / 10_000` and caps it at
 * `amount - 1`, and `settle_payment` refuses while `amount <= invoice_rent +
 * fee` (`InvoiceTooSmall`). The cluster below answers every rent read with
 * 1,000 lamports per byte, so the expected numbers are the program's rule at a
 * rent a test can state.
 */
describe('estimateInvoiceCosts against the program math', () => {
  const policy = {
    feeBpsBelow: 200,
    feeBpsAbove: 200,
    solTierLamports: 0,
    tokenTierBaseUnits: 0,
    flatUsdcBaseUnits: 0,
    flatEurcBaseUnits: 0,
    usdcMint: mint,
    eurcMint: mint,
    feeMode: 'seller' as const
  }

  it('quotes the split the program would settle, in the mint base units', async () => {
    const token = await quote.estimateInvoiceCosts(1_500_000, 'usdc', { mint, seller })

    expect(token.fee).toBe(30_000)
    expect(token.sellerReceives).toBe(1_470_000)
    // Seller-pays: the buyer is charged the price and nothing else.
    expect(token.payerPays).toBe(1_500_000)
    // 262 and 48 bytes at the stub's 1,000 lamports per byte.
    expect(token.invoiceRent).toBe(262_000)
    expect(token.receiptRent).toBe(48_000)
    // A token settle refunds the rents and asks only `amount > fee`.
    expect(token.minPayable).toBe(1)
  })

  it('quotes the SOL floor from `amount > invoice rent + marker rent + fee`', async () => {
    const sol = await quote.estimateInvoiceCosts(10_000_000, 'sol')

    expect(sol.invoiceRent).toBe(230_000)
    expect(sol.receiptRent).toBe(48_000)
    expect(sol.fee).toBe(200_000)
    // The smallest amount that clears the program's `InvoiceTooSmall` guard at
    // 230,000 lamports of invoice rent and 48,000 of marker rent.
    expect(sol.minPayable).toBe(283_674)

    const settles = (amount: number) =>
      amount > sol.invoiceRent + sol.receiptRent + feeOn(policy, amount)
    expect(settles(sol.minPayable)).toBe(true)
    expect(settles(sol.minPayable - 1)).toBe(false)
    expect(settles(10_000_000)).toBe(true)
    // The fee and the permanent marker come out of the seller's side.
    expect(sol.sellerReceives).toBe(9_800_000 - 48_000)
    expect(sol.payerPays).toBe(10_000_000)
  })

  it('quotes a price the program will actually settle', async () => {
    // At the floor the program's own numbers are tight: 5,673 of fee and 48,000
    // of marker rent leave the seller one lamport over the 230,000 invoice rent,
    // which is the rent they get back when the invoice closes.
    const sol = await quote.estimateInvoiceCosts(283_674, 'sol')

    expect(sol.minPayable).toBeLessThanOrEqual(sol.amount)
    expect(sol.fee).toBe(5_673)
    expect(sol.sellerReceives).toBe(230_001)
  })
})
