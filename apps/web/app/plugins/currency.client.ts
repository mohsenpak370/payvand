import type { FiatCurrency } from '~/composables/useFiat'
import { CURRENCY_STORAGE_KEY } from '~/composables/useFiat'

/**
 * Restores the currency this browser last chose, before the first amount is
 * painted. Payment pages are client-only, so this runs ahead of any price and
 * there is no flash of the wrong currency.
 */
export default defineNuxtPlugin(() => {
  const currency = useState<FiatCurrency>('fiat-currency', () => 'usd')
  try {
    const stored = localStorage.getItem(CURRENCY_STORAGE_KEY)
    if (stored === 'usd' || stored === 'eur') currency.value = stored
  } catch {
    // No storage (private mode, embedded webview): keep the default.
  }
})
