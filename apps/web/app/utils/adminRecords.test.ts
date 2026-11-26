import { describe, expect, it } from 'vitest'
import { parsePrice, priceField, recordCurrency, recordPrice } from './adminRecords'

describe('recordPrice', () => {
  it('reads the price in its shop currency', () => {
    expect(recordPrice(12_000_000, 'sol')).toBe('0.012 SOL')
    expect(recordPrice(12_000_000, 'usdc')).toBe('12 USDC')
    expect(recordPrice(4_500_000, 'eurc')).toBe('4.5 EURC')
  })

  it('treats an unknown currency as SOL, the stored default', () => {
    expect(recordCurrency('')).toBe('sol')
    expect(recordPrice(1_000_000_000, 'doge')).toBe('1 SOL')
  })
})

describe('parsePrice', () => {
  it('converts a typed amount to exact base units', () => {
    expect(parsePrice('0.1', 'sol')).toBe(100_000_000)
    expect(parsePrice('4.50', 'usdc')).toBe(4_500_000)
    expect(parsePrice('4,5', 'eurc')).toBe(4_500_000)
    expect(parsePrice('.5', 'usdc')).toBe(500_000)
    expect(parsePrice(' 12 ', 'usdc')).toBe(12_000_000)
  })

  it('refuses zero, junk and more decimals than the currency has', () => {
    expect(parsePrice('0', 'sol')).toBeNull()
    expect(parsePrice('', 'sol')).toBeNull()
    expect(parsePrice('.', 'sol')).toBeNull()
    expect(parsePrice('-1', 'usdc')).toBeNull()
    expect(parsePrice('1e3', 'usdc')).toBeNull()
    expect(parsePrice('0.0000001', 'usdc')).toBeNull()
  })

  it('round-trips the value the edit field starts from', () => {
    expect(parsePrice(priceField(12_345_678, 'sol'), 'sol')).toBe(12_345_678)
    expect(parsePrice(priceField(4_500_000, 'usdc'), 'usdc')).toBe(4_500_000)
  })
})
