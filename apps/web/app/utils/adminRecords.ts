import {
  currencyDecimals,
  currencySymbol,
  formatTokenAmount,
  type SupportedCurrency
} from '../composables/usePaymentApi'

/** A stored currency tag, read safely: anything unknown is treated as SOL, the default. */
export function recordCurrency(raw: string): SupportedCurrency {
  return raw === 'usdc' || raw === 'eurc' ? raw : 'sol'
}

/** `12 USDC`, `0.012 SOL` — a product price in its shop's own units. */
export function recordPrice(baseUnits: number, currency: string): string {
  const code = recordCurrency(currency)
  return `${formatTokenAmount(baseUnits, currencyDecimals(code))} ${currencySymbol(code)}`
}

/** The price as the edit field shows it: units, no symbol. */
export function priceField(baseUnits: number, currency: string): string {
  return formatTokenAmount(baseUnits, currencyDecimals(recordCurrency(currency)))
}

/**
 * What a typed price is in base units, or `null` when it is not a positive
 * amount the currency can count — more decimals than it has, or nothing at all.
 * Parsed from the text rather than through a float, so `0.1` SOL is exactly
 * 100 000 000 lamports.
 */
export function parsePrice(text: string, currency: string): number | null {
  const decimals = currencyDecimals(recordCurrency(currency))
  const match = /^(\d*)(?:[.,](\d*))?$/.exec(text.trim())
  if (!match) return null
  const [, whole = '', fraction = ''] = match
  if (!whole && !fraction) return null
  if (fraction.length > decimals) return null
  const units = Number(whole || '0') * 10 ** decimals + Number(fraction.padEnd(decimals, '0') || '0')
  if (!Number.isSafeInteger(units) || units <= 0) return null
  return units
}
