/**
 * Which of the plan's currencies to offer first, and in what order.
 *
 * The API lists the plan's prices in a fixed order (sol, usdc, eurc). A
 * currency this device can settle silently — an allowance that covers the quote
 * and the session key that signs for it — is worth offering first, because it
 * pays without opening a wallet. Everything else keeps the order the API sent,
 * so the price list is predictable when no allowance applies.
 *
 * Pure, so those two rules are testable without a browser or an RPC.
 */

/** Anything priced in a currency: the API's quote shape, narrowed to what matters. */
export interface PricedQuote<C extends string> {
  currency: C
  amount: number
}

/**
 * The first currency in `silent` for which `silent[currency]` is true, in the
 * API's order, or null when the allowance covers none of them.
 */
export function coveredCurrency<C extends string>(
  quotes: readonly PricedQuote<C>[],
  silent: Partial<Record<C, boolean>>
): C | null {
  for (const quote of quotes) {
    if (silent[quote.currency]) return quote.currency
  }
  return null
}

/**
 * The quotes with the covered currency moved to the front, keeping the API's
 * order for the rest. A null or unknown currency returns the list unchanged,
 * and the input is never mutated.
 */
export function orderedQuotes<T extends { currency: string }>(
  quotes: readonly T[],
  covered: string | null
): T[] {
  const index = covered ? quotes.findIndex(quote => quote.currency === covered) : -1
  if (index <= 0) return [...quotes]
  return [quotes[index]!, ...quotes.slice(0, index), ...quotes.slice(index + 1)]
}
