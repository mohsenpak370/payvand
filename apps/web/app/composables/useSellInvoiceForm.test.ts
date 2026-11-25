import type { Ref } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { invoiceCosts, type FeePolicy, type InvoiceCosts } from '~/utils/invoiceCost'
import {
  lamportsToSol,
  solToLamports,
  type CreatePaymentInput,
  type DiscountCode,
  type SupportedCurrency
} from './usePaymentApi'
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
    allowDiscounts: ref(false),
    priceCurrency: ref(currency),
    applyCreated: () => {}
  })
}

/**
 * A discount code is offered where it can price something: only a payment built
 * from the shop's products, with products actually on the tray. A typed price,
 * or an empty cart, hides the discount sections.
 */
describe('where a discount code may be attached', () => {
  const product = {
    id: 'p1',
    shop_id: 'shop-1',
    name: 'Flat white',
    details: '',
    category: '',
    price_lamports: 5_000_000,
    created_at_unix: 0
  }

  it('offers one on the shared form only for a cart with products', () => {
    const form = sellFormIn('sol')
    expect(form.discountAllowed.value).toBe(false)

    form.priceMode.value = 'products'
    expect(form.discountAllowed.value).toBe(false)

    form.cart.value = [{ product, quantity: 1 }]
    expect(form.discountAllowed.value).toBe(true)
  })

  it('offers one on a shop\'s own page only while its menu fills the payment', () => {
    const form = useSellInvoiceForm({
      cluster: 'devnet',
      shopsEnabled: ref(true),
      shopId: 'shop-1',
      selectedDiscount: ref(null),
      allowDiscounts: ref(false),
      priceCurrency: ref('sol'),
      applyCreated: () => {}
    })

    expect(form.priceMode.value).toBe('products')
    expect(form.discountAllowed.value).toBe(false)

    form.cart.value = [{ product, quantity: 1 }]
    expect(form.discountAllowed.value).toBe(true)

    form.priceMode.value = 'amount'
    expect(form.discountAllowed.value).toBe(false)
  })
})

/**
 * What the invoice lets the buyer do with the seller's codes. The flag rides
 * in the QR code, and the payment page offers the buyer nothing else, so the
 * body sent to the API is the whole of this rule.
 */
describe('the invoice that invites the buyer\'s own code', () => {
  const seller = 'So11111111111111111111111111111111111111112'
  const product = {
    id: 'p1',
    shop_id: 'shop-1',
    name: 'Flat white',
    details: '',
    category: '',
    price_lamports: 5_000_000,
    created_at_unix: 0
  }
  const code: DiscountCode = { id: 'd1', code: 'SAVE10', percent_off: 10 }

  let created: CreatePaymentInput[]

  beforeEach(() => {
    created = []
    vi.stubGlobal('usePaymentApi', () => ({
      createPayment: async (body: CreatePaymentInput) => {
        created.push(body)
        return { payment: { items: [] }, qrPayload: '{}' }
      }
    }))
    vi.stubGlobal('useSolanaWallet', () => ({ publicKey: ref(seller), connected: ref(true) }))
  })

  /** Creates the invoice the form describes and returns what it posted. */
  async function post(options: {
    shopId?: string
    fromProducts?: boolean
    typedAmount?: boolean
    allowDiscounts?: boolean
    discount?: DiscountCode | null
  }) {
    const form = useSellInvoiceForm({
      cluster: 'devnet',
      shopsEnabled: ref(true),
      shopId: options.shopId,
      selectedDiscount: ref(options.discount ?? null),
      allowDiscounts: ref(options.allowDiscounts ?? false),
      priceCurrency: ref('sol'),
      applyCreated: () => {}
    })
    if (options.typedAmount) {
      form.priceMode.value = 'amount'
    }
    if (options.fromProducts) {
      form.priceMode.value = 'products'
      form.cart.value = [{ product, quantity: 2 }]
    }
    await form.createPayment()
    return created[0]!
  }

  it('invites one on a payment built from products', async () => {
    const body = await post({ fromProducts: true, allowDiscounts: true })

    expect(body.allow_discounts).toBe(true)
    expect(body.items).toHaveLength(1)
  })

  it('stays silent when the seller left the switch off', async () => {
    const body = await post({ fromProducts: true })

    expect(body.allow_discounts).toBe(false)
  })

  it('never invites one beside a code the seller attached', async () => {
    const body = await post({ fromProducts: true, allowDiscounts: true, discount: code })

    expect(body.discount_code).toBe('SAVE10')
    expect(body.allow_discounts).toBe(false)
  })

  it('never invites one on a typed price, even on a shop\'s own page', async () => {
    const body = await post({ shopId: 'shop-1', typedAmount: true, allowDiscounts: true })

    expect(body.allow_discounts).toBe(false)
    expect(body.discount_code).toBeUndefined()
  })
})

describe('the sell form\'s net quote', () => {
  it('quotes a USDC price with the USDC mint, so the flat is charged', () => {
    sellFormIn('usdc')

    expect(estimateInvoiceCosts).toHaveBeenCalledWith(10_000, 'usdc', {
      mint: USDC_MINT,
      cluster: 'devnet'
    })
  })

  it('quotes a EURC price with the EURC mint', () => {
    sellFormIn('eurc')

    expect(estimateInvoiceCosts).toHaveBeenCalledWith(10_000, 'eurc', {
      mint: EURC_MINT,
      cluster: 'devnet'
    })
  })

  it('quotes a SOL price with no mint, which pays no flat', () => {
    sellFormIn('sol')

    expect(estimateInvoiceCosts).toHaveBeenCalledWith(10_000_000, 'sol', {
      mint: undefined,
      cluster: 'devnet'
    })
  })
})
