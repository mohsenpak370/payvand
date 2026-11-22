import { describe, expect, it } from 'vitest'
import { fiatValueOf, type SolRates } from './useFiat'

// 1 SOL = $100 = €90, so the two fiat currencies relate 0.9 to 1.
const rates: SolRates = { usd: 100, eur: 90, at: '2026-11-22T00:00:00Z' }

describe('fiatValueOf', () => {
  it('prices lamports through the picked fiat', () => {
    expect(fiatValueOf('sol', 500_000_000, rates, 'usd')).toBe(50)
    expect(fiatValueOf('sol', 500_000_000, rates, 'eur')).toBe(45)
  })

  it('takes a stablecoin at its own denomination', () => {
    expect(fiatValueOf('usdc', 5_000_000, rates, 'usd')).toBe(5)
    expect(fiatValueOf('eurc', 5_000_000, rates, 'eur')).toBe(5)
  })

  it('crosses USDC and EURC through the SOL rate', () => {
    // $10 is €9, and €10 is $11.11…
    expect(fiatValueOf('usdc', 10_000_000, rates, 'eur')).toBeCloseTo(9)
    expect(fiatValueOf('eurc', 10_000_000, rates, 'usd')).toBeCloseTo(11.111, 3)
  })

  it('has no answer without the rate it needs', () => {
    expect(fiatValueOf('sol', 1_000_000_000, null, 'usd')).toBeNull()
    expect(fiatValueOf('eurc', 1_000_000, { usd: 100, eur: 0, at: '' }, 'usd')).toBeNull()
  })
})
