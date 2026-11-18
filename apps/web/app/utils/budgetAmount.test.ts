import { describe, expect, it } from 'vitest'
import { budgetAmountLabel, budgetBaseUnits } from './budgetAmount'

/**
 * The limit field is the one place a person types an amount, and every other
 * number on the card — the quote, the buttons, the messages — is derived from
 * it. Both sides have to agree on what the typed number means, or a limit of
 * `1` would be one lamport in a sentence and one whole SOL on chain.
 */

describe('budgetBaseUnits', () => {
  it('turns SOL into lamports', () => {
    // The default the card starts at: 0.1 SOL is the low limit it recommends.
    expect(budgetBaseUnits('sol', 0.1)).toBe(100_000_000)
    expect(budgetBaseUnits('sol', 1)).toBe(1_000_000_000)
  })

  it('turns a stablecoin into six-decimal atoms', () => {
    expect(budgetBaseUnits('usdc', 1)).toBe(1_000_000)
    expect(budgetBaseUnits('eurc', 0.5)).toBe(500_000)
  })

  it('rounds to the smallest unit instead of leaving a fraction of one', () => {
    // Typing more decimals than the currency has is a rounding, not an error:
    // the quote and the transaction both need whole base units.
    expect(budgetBaseUnits('sol', 0.0000000004)).toBe(0)
    expect(budgetBaseUnits('usdc', 0.0000004)).toBe(0)
  })
})

describe('budgetAmountLabel', () => {
  it('writes each amount in the currency it is held in', () => {
    expect(budgetAmountLabel('sol', 100_000_000)).toBe('0.1 SOL')
    expect(budgetAmountLabel('usdc', 1_500_000)).toBe('1.5 USDC')
    expect(budgetAmountLabel('eurc', 2_000_000)).toBe('2 EURC')
  })

  it('drops trailing zeros the way the rest of the app does', () => {
    expect(budgetAmountLabel('sol', 1_000_000_000)).toBe('1 SOL')
    expect(budgetAmountLabel('usdc', 0)).toBe('0 USDC')
  })
})
