import { describe, expect, it } from 'vitest'
import { coveredCurrency, orderedQuotes } from './subscriptionQuotes'

type Currency = 'sol' | 'usdc' | 'eurc'

/** The API's order, with a euro plan's round numbers as prices. */
const quotes = [
  { currency: 'sol' as Currency, amount: 25_668_127 },
  { currency: 'usdc' as Currency, amount: 1_860_141 },
  { currency: 'eurc' as Currency, amount: 1_700_000 }
]

const currencies = (list: { currency: string }[]) => list.map(quote => quote.currency)

describe('coveredCurrency', () => {
  it('names the first currency this device can pay silently', () => {
    expect(coveredCurrency(quotes, { usdc: true })).toBe('usdc')
    expect(coveredCurrency(quotes, { sol: true, usdc: true })).toBe('sol')
  })

  it('covers nothing when the allowance is short or unread', () => {
    expect(coveredCurrency(quotes, {})).toBeNull()
    expect(coveredCurrency(quotes, { sol: false, usdc: false, eurc: false })).toBeNull()
    expect(coveredCurrency(quotes, { sol: undefined })).toBeNull()
  })

  it('covers nothing when the plan has no prices', () => {
    expect(coveredCurrency([], { sol: true })).toBeNull()
  })

  it('ignores a currency the plan does not sell', () => {
    expect(coveredCurrency(quotes, { eurc: true, usdc: true })).toBe('usdc')
  })
})

describe('orderedQuotes', () => {
  it('moves the covered currency first and keeps the rest in API order', () => {
    expect(currencies(orderedQuotes(quotes, 'usdc'))).toEqual(['usdc', 'sol', 'eurc'])
    expect(currencies(orderedQuotes(quotes, 'eurc'))).toEqual(['eurc', 'sol', 'usdc'])
  })

  it('leaves the API order alone when nothing is covered', () => {
    expect(currencies(orderedQuotes(quotes, null))).toEqual(['sol', 'usdc', 'eurc'])
  })

  it('leaves the order alone when the first currency is already covered', () => {
    expect(currencies(orderedQuotes(quotes, 'sol'))).toEqual(['sol', 'usdc', 'eurc'])
  })

  it('leaves the order alone when the named currency is not sold', () => {
    expect(currencies(orderedQuotes(quotes, 'eurc' as Currency))).toEqual(['eurc', 'sol', 'usdc'])
    expect(currencies(orderedQuotes(quotes, 'jpy' as Currency))).toEqual(['sol', 'usdc', 'eurc'])
  })

  it('hands back a copy, so the caller cannot reorder the source list', () => {
    const ordered = orderedQuotes(quotes, 'eurc')
    expect(ordered).not.toBe(quotes)
    expect(currencies(quotes)).toEqual(['sol', 'usdc', 'eurc'])
    expect(ordered[0]).toBe(quotes[2])
  })

  it('handles an empty plan', () => {
    expect(orderedQuotes([], 'sol')).toEqual([])
  })
})
