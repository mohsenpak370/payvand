/**
 * Prices are charged on chain in SOL, but people think in their own money. This
 * is the one place that knows the rate and how to turn a lamport amount into a
 * readable price in the currency the person picked.
 */

export type FiatCurrency = 'usd' | 'eur'

export interface SolRates {
  usd: number
  eur: number
  /** When the rate was read, ISO-8601. */
  at: string
}

export const CURRENCY_STORAGE_KEY = 'payvand.currency.v1'

export const FIAT_CURRENCIES: FiatCurrency[] = ['usd', 'eur']

/**
 * The currency a person reads prices in. Shared state, so changing it on the
 * profile page also changes the invoices they open in another tab of this app.
 */
export function useCurrencyPreference() {
  const currency = useState<FiatCurrency>('fiat-currency', () => 'usd')

  function setCurrency(next: FiatCurrency) {
    currency.value = next
    try {
      localStorage.setItem(CURRENCY_STORAGE_KEY, next)
    } catch {
      // Private mode without storage: the choice still holds for this visit.
    }
  }

  return { currency, setCurrency }
}

/**
 * The current SOL price, read once per page from the app's own cached route.
 * `null` until it arrives, and it stays `null` when the rate is unavailable —
 * callers show no fiat hint rather than a wrong one.
 */
export function useSolRates() {
  const { data } = useFetch<SolRates>('/api/rates', {
    key: 'sol-rates',
    server: false,
    lazy: true
  })
  return data
}

/** `9.3` → `$9.30`; small amounts keep more digits so they do not read as zero. */
export function formatFiat(value: number, currency: FiatCurrency): string {
  const digits = value > 0 && value < 1 ? 4 : 2
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: currency.toUpperCase(),
    minimumFractionDigits: digits === 2 ? 2 : 0,
    maximumFractionDigits: digits
  }).format(value)
}

/** The fiat value of a lamport price, or `null` while the rate is unknown. */
export function fiatFromLamports(
  lamports: number,
  rates: SolRates | null | undefined,
  currency: FiatCurrency
): string | null {
  const rate = rates?.[currency]
  if (!rate || !Number.isFinite(lamports)) return null
  return formatFiat((lamports / 1_000_000_000) * rate, currency)
}

/**
 * What an amount already held in `currency` is worth, in the currency the
 * person reads prices in. SOL goes through its rate; a stablecoin is already
 * fiat — USDC is dollars and EURC is euros, so an amount in the same one needs
 * no conversion at all, and crossing between the two goes through the SOL rate,
 * the only ratio the app knows.
 *
 * `null` while a rate it needs is unknown, so a caller shows nothing rather
 * than a number that is quietly wrong.
 */
export function fiatValueOf(
  currency: 'sol' | 'usdc' | 'eurc',
  baseUnits: number,
  rates: SolRates | null | undefined,
  display: FiatCurrency
): number | null {
  if (!Number.isFinite(baseUnits)) return null
  if (currency === 'sol') {
    const rate = rates?.[display]
    return rate ? (baseUnits / 1_000_000_000) * rate : null
  }
  const amount = baseUnits / 1_000_000
  const source: FiatCurrency = currency === 'usdc' ? 'usd' : 'eur'
  if (source === display) return amount
  const usd = rates?.usd
  const eur = rates?.eur
  if (!usd || !eur) return null
  return source === 'usd' ? amount * (eur / usd) : amount * (usd / eur)
}

/** Lamports for a fiat price, or 0 while the rate is unknown. */
export function lamportsFromFiat(
  value: number,
  rates: SolRates | null | undefined,
  currency: FiatCurrency
): number {
  const rate = rates?.[currency]
  if (!rate || !Number.isFinite(value) || value <= 0) return 0
  return Math.round((value / rate) * 1_000_000_000)
}
