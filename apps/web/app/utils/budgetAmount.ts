import {
  currencyDecimals,
  currencySymbol,
  formatTokenAmount,
  solToLamports,
  type SupportedCurrency
} from '../composables/usePaymentApi'

/**
 * The budget card shows one currency at a time, and every amount on it — the
 * figures, the buttons and the sentences — is in that currency's own units.
 * Keeping the conversion here is what makes the field, the quote and the
 * messages agree on what `0.1` means.
 */

/** `0.1 SOL`, `5 USDC` — a base-unit amount written the way the card shows it. */
export function budgetAmountLabel(currency: SupportedCurrency, amount: number): string {
  return `${formatTokenAmount(amount, currencyDecimals(currency))} ${currencySymbol(currency)}`
}

/** What a typed amount is in the currency's base units: lamports or token atoms. */
export function budgetBaseUnits(currency: SupportedCurrency, amount: number): number {
  return currency === 'sol' ? solToLamports(amount) : Math.round(amount * 1_000_000)
}
