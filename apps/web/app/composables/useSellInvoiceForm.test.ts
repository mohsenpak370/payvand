import type { Ref } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { invoiceCosts, type FeePolicy, type InvoiceCosts } from '~/utils/invoiceCost'
import { lamportsToSol, solToLamports, type SupportedCurrency } from './usePaymentApi'
import { createReference } from './usePaymentProgram'
import { useSellInvoiceForm } from './useSellInvoiceForm'

/**
 * The seller's net comes from `estimateInvoiceCosts`, and that quote only
 * charges a currency's flat when it is told which mint the invoice pays in. A
 * sell form that quoted without one showed a USDC/EURC seller 89,400 base units
 * more than the program would hand them.
 */

const USDC_MINT = '4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU'
const EURC_MINT = 'HzwqbKZw8HxMN6bF2yFZNrht3c2iXXzpKcFu7uBEDKtr'

const policy: FeePolicy = {
  feeBpsBelow: 100,
  feeBpsAbove: 50,
  solTierLamports: 100_000_000,
  tokenTierBaseUnits: 10_000_000,
  flatUsdcBaseUnits: 89_400,
  flatEurcBaseUnits: 89_400,
  usdcMint: USDC_MINT,
  eurcMint: EURC_MINT,
  feeMode: 'seller'
}

const quote = (currency: SupportedCurrency, mint: string): InvoiceCosts => invoiceCosts({
  policy,
  amount: 10_000,
  currency,
  mint,
  invoiceRent: 1_000_000,
  receiptRent: 1_244_960
})

// Just enough Nuxt reactivity for a form that is only read. The cast is the
// missing `[RefSymbol]` brand: the form never subscribes, it only reads.
const ref = <T>(value: T) => ({ value }) as unknown as Ref<T>
const computed = <T>(compute: () => T) => ({
  get value(): T {
    return compute()
  }
})

let estimateInvoiceCosts: ReturnType<typeof vi.fn>

beforeEach(() => {
  estimateInvoiceCosts = vi.fn(async (_amount: number, currency: SupportedCurrency) =>
    quote(currency, currency === 'eurc' ? EURC_MINT : USDC_MINT)
  )

  vi.stubGlobal('ref', ref)
  vi.stubGlobal('computed', computed)
  vi.stubGlobal('watch', () => {})
  // The quote is fired from `onMounted`, so the stub runs it at construction.
  vi.stubGlobal('onMounted', (callback: () => void) => callback())
  vi.stubGlobal('useI18n', () => ({ t: (key: string) => key }))
  vi.stubGlobal('appT', (key: string) => key)
  vi.stubGlobal('usePaymentApi', () => ({ createPayment: async () => ({}) }))
  vi.stubGlobal('useShopApi', () => ({ list: async () => [] }))
  vi.stubGlobal('useSolanaWallet', () => ({ publicKey: ref(null), connected: ref(false) }))
  vi.stubGlobal('useSellerTokenAccount', () => ({ ensureSellerTokenAccount: async () => {} }))
  vi.stubGlobal('useCurrencyPreference', () => ({ currency: ref('sol') }))
  vi.stubGlobal('useInvoiceCosts', () => ({ estimateInvoiceCosts }))
  vi.stubGlobal('solToLamports', solToLamports)
  vi.stubGlobal('lamportsToSol', lamportsToSol)
  vi.stubGlobal('createReference', createReference)
})

afterEach(() => {
  vi.unstubAllGlobals()
})

/** The form priced at the default `0.01`, in the currency it charges. */
function sellFormIn(currency: SupportedCurrency) {
  return useSellInvoiceForm({
    cluster: 'devnet',
    shopsEnabled: ref(true),
    selectedDiscount: ref(null),
    priceCurrency: ref(currency),
    applyCreated: () => {}
  })
}

describe('the sell form\'s net quote', () => {
  it('quotes a USDC price with the USDC mint, so the flat is charged', () => {
    sellFormIn('usdc')

    expect(estimateInvoiceCosts).toHaveBeenCalledWith(10_000, 'usdc', { mint: USDC_MINT })
  })

  it('quotes a EURC price with the EURC mint', () => {
    sellFormIn('eurc')

    expect(estimateInvoiceCosts).toHaveBeenCalledWith(10_000, 'eurc', { mint: EURC_MINT })
  })

  it('quotes a SOL price with no mint, which pays no flat', () => {
    sellFormIn('sol')

    expect(estimateInvoiceCosts).toHaveBeenCalledWith(10_000_000, 'sol', { mint: undefined })
  })
})
