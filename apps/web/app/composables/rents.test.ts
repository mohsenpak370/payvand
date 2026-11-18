import { address } from '@vue-solana/nuxt/kit'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { clearRentCache, rentMinimum } from './rents'
import { clearBudgetAccountCache, useBudget } from './useBudget'

/**
 * A rent is a property of the cluster's rent parameters and the account's size,
 * so the same size has the same answer everywhere. The page nevertheless asks
 * for the same few sizes from more than one quote — the checkout's invoice and
 * receipt, the allowance page's budget and token budget — and `useBudget` used
 * to answer them from a `Map` created inside the composable, so every instance
 * started empty and asked again.
 *
 * The cache is module level now and both quote paths share it; these tests pin
 * that, including the one route no per-instance cache can take: a second
 * `useBudget()` and a second quote, served for free.
 */

type Client = ReturnType<typeof useSolanaClient>['client']

const buyer = address('B8V5fS6rPD9P9Hhw6xYJ2Kf7UEfFQHhBb4HfCxRxJJPb')

let rentCalls: number[]

/**
 * A cluster that records every rent it is asked for, and nothing on it: no
 * budget, no session key balance. Only what the quote needs to be asked for.
 */
function fakeClient(): Client {
  return {
    rpc: {
      getMinimumBalanceForRentExemption: (space: bigint) => ({
        send: async () => {
          rentCalls.push(Number(space))
          return Number(space) * 1_000
        }
      }),
      getAccountInfo: () => ({ send: async () => ({ value: null }) }),
      getBalance: () => ({ send: async () => ({ value: 0n }) })
    }
  } as unknown as Client
}

beforeEach(() => {
  clearRentCache()
  clearBudgetAccountCache()
  rentCalls = []
})

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

describe('rent quotes', () => {
  it('answers a repeated size from the cache, whoever asks', async () => {
    const client = fakeClient()

    expect(await rentMinimum(client, 301)).toBe(301_000)
    expect(await rentMinimum(client, 301)).toBe(301_000)

    expect(rentCalls).toEqual([301])
  })

  it('keeps one answer per size, not one answer per quote', async () => {
    const client = fakeClient()

    await rentMinimum(client, 230)
    await rentMinimum(client, 262)
    await rentMinimum(client, 230)

    expect(rentCalls).toEqual([230, 262])
  })

  it('asks again once the minute is up', async () => {
    vi.useFakeTimers({ now: 1_760_000_000_000 })
    const client = fakeClient()

    await rentMinimum(client, 97)
    vi.setSystemTime(Date.now() + 61_000)
    await rentMinimum(client, 97)

    expect(rentCalls).toEqual([97, 97])
  })

  it('asks again after the cache is cleared', async () => {
    const client = fakeClient()

    await rentMinimum(client, 129)
    clearRentCache()
    await rentMinimum(client, 129)

    expect(rentCalls).toEqual([129, 129])
  })

  it('serves the allowance page the same answer, across instances', async () => {
    vi.stubGlobal('appT', (key: string) => key)
    vi.stubGlobal('useSolanaClient', () => ({ client: fakeClient() }))
    // The float comes from `Config` on chain; this test is about the rent.
    vi.stubGlobal('useFeeConfig', () => ({
      load: async () => ({ sessionFeeFloat: 12_000_000 })
    }))
    // A global from the app's Nuxt config, not a module import.
    vi.stubGlobal('NETWORK_FEE_LAMPORTS', 5_000)

    const first = await useBudget().openBudgetCost(buyer, 5_000_000)
    const second = await useBudget().openBudgetCost(buyer, 5_000_000)

    // `BUDGET_SPACE` is asked for once: the second instance was a fresh
    // composable, but not a fresh cache, and the second quote added nothing.
    expect(rentCalls).toEqual([97])
    expect(first.rent).toBe(97_000)
    expect(second).toEqual(first)
  })
})
