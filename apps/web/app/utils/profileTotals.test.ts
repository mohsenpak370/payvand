import { describe, expect, it } from 'vitest'
import type { MyTransaction } from '~/composables/useAuth'
import type { SolRates } from '~/composables/useFiat'
import { fiatOf, summarizeFlow } from './profileTotals'

const rates: SolRates = { usd: 200, eur: 100, at: '2026-11-23T00:00:00Z' }

function item(overrides: Partial<MyTransaction>): MyTransaction {
  return {
    id: 'invoice',
    direction: 'sent',
    counterparty: 'counterparty',
    currency: 'sol',
    amount: 1_000_000_000,
    decimals: 9,
    amount_lamports: 1_000_000_000,
    memo: '',
    name: '',
    description: '',
    status: 'settled',
    created_at_unix: 1_700_000_000,
    settled_at_unix: 1_700_000_100,
    ...overrides
  }
}

describe('summarizeFlow', () => {
  it('adds each settled payment to the coin it was paid in', () => {
    const flow = summarizeFlow(
      [
        item({ amount: 2_000_000_000, amount_lamports: 2_000_000_000 }),
        item({ direction: 'received', amount: 500_000_000, amount_lamports: 500_000_000 }),
        item({ currency: 'usdc', amount: 5_000_000, amount_lamports: 0, decimals: 6 }),
        item({ currency: 'eurc', amount: 3_000_000, amount_lamports: 0, decimals: 6 })
      ],
      rates,
      'usd'
    )

    expect(flow.spent.coins.sol).toBe(2_000_000_000)
    expect(flow.received.coins.sol).toBe(500_000_000)
    expect(flow.spent.coins.usdc).toBe(5_000_000)
    expect(flow.spent.coins.eurc).toBe(3_000_000)
    expect(flow.received.coins.eurc).toBe(0)
    expect(flow.net.coins.sol).toBe(-1_500_000_000)
  })

  it('leaves unpaid invoices out of the totals', () => {
    const flow = summarizeFlow(
      [
        item({ status: 'pending', amount: 9_000_000_000 }),
        item({ status: 'missing', amount: 9_000_000_000 }),
        item({ amount: 1_000_000_000 })
      ],
      rates,
      'usd'
    )

    expect(flow.spent.coins.sol).toBe(1_000_000_000)
  })

  it('converts every coin into the display currency', () => {
    const flow = summarizeFlow(
      [
        item({ amount: 1_000_000_000, amount_lamports: 1_000_000_000 }),
        item({ currency: 'usdc', amount: 5_000_000, amount_lamports: 0, decimals: 6 })
      ],
      rates,
      'usd'
    )

    // 1 SOL at 200 USD, plus 5 USDC at par.
    expect(flow.spent.fiat).toBe(205)
  })

  it('reports net flow in both coins and money', () => {
    const flow = summarizeFlow(
      [
        item({ direction: 'received', amount: 1_000_000_000, amount_lamports: 1_000_000_000 }),
        item({ amount: 250_000_000, amount_lamports: 250_000_000 })
      ],
      rates,
      'eur'
    )

    // 1 SOL in at 100 EUR, 0.25 SOL out at the same rate.
    expect(flow.net.fiat).toBe(75)
    expect(flow.net.coins.sol).toBe(750_000_000)
  })

  it('keeps the coins when a rate is missing but drops the money figure', () => {
    const flow = summarizeFlow(
      [item({ amount: 1_000_000_000, amount_lamports: 1_000_000_000 })],
      null,
      'usd'
    )

    expect(flow.spent.coins.sol).toBe(1_000_000_000)
    expect(flow.spent.fiat).toBeNull()
    expect(flow.net.fiat).toBeNull()
  })
})

describe('fiatOf', () => {
  it('is zero, not unknown, for a side with nothing in it', () => {
    expect(fiatOf({ sol: 0, usdc: 0, eurc: 0 }, null, 'usd')).toBe(0)
  })

  it('prices a euro balance in dollars through the SOL rate', () => {
    // EUR/USD is rates.usd / rates.eur, so 2 EURC is 4 USD here.
    expect(fiatOf({ sol: 0, usdc: 0, eurc: 2_000_000 }, rates, 'usd')).toBe(4)
  })
})
