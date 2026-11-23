import type { MyTransaction } from '~/composables/useAuth'
import type { FiatCurrency, SolRates } from '~/composables/useFiat'
import { fiatValueOf } from '~/composables/useFiat'

/**
 * What left and what arrived, in the three currencies the app settles in, with
 * one figure in the money the person reads prices in.
 *
 * Only settled payments count: a pending invoice is a request, not money, and
 * counting it would move the totals every time someone opens a QR they never
 * paid.
 */

export type FlowCurrency = 'sol' | 'usdc' | 'eurc'

/** Base units per coin: lamports for SOL, six-decimal units for the stables. */
export interface CoinTotals {
  sol: number
  usdc: number
  eurc: number
}

export interface FlowSide {
  coins: CoinTotals
  /** The whole side in the display currency; null while a rate it needs is unknown. */
  fiat: number | null
}

export interface FlowSummary {
  spent: FlowSide
  received: FlowSide
  net: FlowSide
}

export const FLOW_CURRENCIES: FlowCurrency[] = ['sol', 'usdc', 'eurc']

function emptyCoins(): CoinTotals {
  return { sol: 0, usdc: 0, eurc: 0 }
}

/** Narrows an item's currency to one this app prices; anything else is SOL. */
export function flowCurrencyOf(item: MyTransaction): FlowCurrency {
  return item.currency === 'usdc' || item.currency === 'eurc' ? item.currency : 'sol'
}

/**
 * Converts a set of coin totals into the display currency. A side with nothing
 * in it is worth exactly zero; a side whose rate is missing is `null`, so the
 * card shows no number rather than one that is quietly wrong.
 */
export function fiatOf(
  coins: CoinTotals,
  rates: SolRates | null | undefined,
  display: FiatCurrency
): number | null {
  let total = 0
  for (const currency of FLOW_CURRENCIES) {
    const baseUnits = coins[currency]
    if (!baseUnits) continue
    const converted = fiatValueOf(currency, baseUnits, rates, display)
    if (converted === null) return null
    total += converted
  }
  return total
}

export function summarizeFlow(
  items: MyTransaction[],
  rates: SolRates | null | undefined,
  display: FiatCurrency
): FlowSummary {
  const spent = emptyCoins()
  const received = emptyCoins()

  for (const item of items) {
    if (item.status !== 'settled') continue
    const target = item.direction === 'received' ? received : spent
    target[flowCurrencyOf(item)] += item.amount
  }

  const net = emptyCoins()
  for (const currency of FLOW_CURRENCIES) {
    net[currency] = received[currency] - spent[currency]
  }

  const spentFiat = fiatOf(spent, rates, display)
  const receivedFiat = fiatOf(received, rates, display)

  return {
    spent: { coins: spent, fiat: spentFiat },
    received: { coins: received, fiat: receivedFiat },
    net: {
      coins: net,
      fiat: spentFiat === null || receivedFiat === null ? null : receivedFiat - spentFiat
    }
  }
}
