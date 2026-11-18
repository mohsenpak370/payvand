import { address, getAddressEncoder } from '@vue-solana/nuxt/kit'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { accountDiscriminator } from './usePaymentProgram'
import { clearBudgetAccountCache, useBudget } from './useBudget'

/**
 * Reading a budget or its vault from the cluster is pure observation: a currency
 * toggle, a repeated quote or a second caller asks for the same account again
 * within a click, and nothing on chain can have changed in between. The reads
 * are held for a minute so those paths cost one round-trip, but a forced reload
 * (after a mutation) and the minute's end both read again.
 */
describe('budget account reads', () => {
  const buyer = address('B8V5fS6rPD9P9Hhw6xYJ2Kf7UEfFQHhBb4HfCxRxJJPb')
  const sessionKey = address('3rpBzUBbZPXFKhGywCRduD3hrREDjvDtaw2Mos2yUUPx')
  const mint = address('4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU')

  let accountInfoCalls: string[]
  let tokenBalanceCalls: string[]
  let budgetBytes: string | null
  let tokenBudgetBytes: string | null
  let vaultAmount: string

  async function encodeBudgetAccount(): Promise<string> {
    const bytes = new Uint8Array(97)
    const encoder = getAddressEncoder()
    bytes.set(await accountDiscriminator('Budget'), 0)
    bytes.set(encoder.encode(buyer), 8)
    bytes.set(encoder.encode(sessionKey), 40)
    const view = new DataView(bytes.buffer)
    view.setBigUint64(72, 100_000_000n, true)
    view.setBigUint64(80, 10_000_000n, true)
    view.setBigUint64(88, 24_288n, true)
    bytes[96] = 255
    return Buffer.from(bytes).toString('base64')
  }

  async function encodeTokenBudgetAccount(): Promise<string> {
    const bytes = new Uint8Array(129)
    const encoder = getAddressEncoder()
    bytes.set(await accountDiscriminator('TokenBudget'), 0)
    bytes.set(encoder.encode(buyer), 8)
    bytes.set(encoder.encode(sessionKey), 40)
    bytes.set(encoder.encode(mint), 72)
    const view = new DataView(bytes.buffer)
    view.setBigUint64(104, 2_000_000n, true)
    view.setBigUint64(112, 500_000n, true)
    view.setBigUint64(120, 24_288n, true)
    bytes[128] = 255
    return Buffer.from(bytes).toString('base64')
  }

  beforeEach(async () => {
    clearBudgetAccountCache()
    accountInfoCalls = []
    tokenBalanceCalls = []
    budgetBytes = null
    tokenBudgetBytes = null
    vaultAmount = '0'

    vi.stubGlobal('appT', (key: string) => key)
    vi.stubGlobal('useFeeConfig', () => ({ load: async () => ({ sessionFeeFloat: 12_000_000 }) }))
    vi.stubGlobal('useSolanaClient', () => ({
      client: {
        rpc: {
          getAccountInfo: (account: string) => ({
            send: async () => {
              accountInfoCalls.push(account)
              // A caller reads one budget kind per test; hand back whichever bytes
              // this test armed, and nothing when it armed neither.
              const armed = budgetBytes ?? tokenBudgetBytes
              return { value: armed ? { data: [armed], lamports: 2_000_000n } : null }
            }
          }),
          getTokenAccountBalance: (account: string) => ({
            send: async () => {
              tokenBalanceCalls.push(account)
              return { value: { amount: vaultAmount } }
            }
          })
        }
      }
    }))
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.unstubAllGlobals()
  })

  it('reads a missing budget once per buyer within the minute', async () => {
    const budget = useBudget()

    expect(await budget.loadBudget(buyer)).toBeNull()
    expect(await budget.loadBudget(buyer)).toBeNull()

    expect(accountInfoCalls).toHaveLength(1)
  })

  it('reads an existing budget once per buyer within the minute', async () => {
    budgetBytes = await encodeBudgetAccount()
    const budget = useBudget()

    const first = await budget.loadBudget(buyer)
    const second = await budget.loadBudget(buyer)

    expect(first?.monthlyLimit).toBe(100_000_000)
    expect(second).toEqual(first)
    expect(accountInfoCalls).toHaveLength(1)
  })

  it('reads a token budget and its vault once per buyer and mint within the minute', async () => {
    tokenBudgetBytes = await encodeTokenBudgetAccount()
    vaultAmount = '1_250_000'.replaceAll('_', '')
    const budget = useBudget()

    const first = await budget.loadTokenBudget(buyer, mint)
    const second = await budget.loadTokenBudget(buyer, mint)

    expect(first?.amount).toBe(1_250_000)
    expect(first?.monthlyLimit).toBe(2_000_000)
    expect(second).toEqual(first)
    // The budget account and its vault are read once together, then served from
    // the cache — a currency toggle must not ask for either again.
    expect(accountInfoCalls).toHaveLength(1)
    expect(tokenBalanceCalls).toHaveLength(1)
  })

  it('reads again when forced, so a mutation sees the new state', async () => {
    const budget = useBudget()

    await budget.loadBudget(buyer)
    await budget.loadBudget(buyer, { force: true })

    expect(accountInfoCalls).toHaveLength(2)
  })

  it('reads again once the minute is up', async () => {
    vi.useFakeTimers({ now: 1_760_000_000_000 })
    const budget = useBudget()

    await budget.loadBudget(buyer)
    vi.setSystemTime(Date.now() + 61_000)
    await budget.loadBudget(buyer)

    expect(accountInfoCalls).toHaveLength(2)
  })
})
