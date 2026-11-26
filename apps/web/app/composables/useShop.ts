/**
 * The seller's shops and their product catalogues.
 *
 * Nothing here is on chain. A shop is a menu the payment page prices an invoice
 * from, and the receipt it produces travels inside the QR code — the API still
 * keeps no invoices, so a product is only a starting price the seller can pick
 * instead of typing.
 */

import type { SupportedCurrency } from '~/composables/usePaymentApi'
import { currencySymbol, formatTokenAmount } from '~/composables/usePaymentApi'

export interface Shop {
  id: string
  name: string
  description: string
  /**
   * What the menu is priced — and charged — in. The shop's own payment page
   * prices its invoice in it, so a shop in EURC is paid in EURC.
   */
  currency: SupportedCurrency
}

/**
 * One price, written in the unit its shop charges in. Every stored price is in
 * that currency's own base units, so this is the only place that turns one into
 * money a person reads.
 */
export function priceInCurrency(amount: number, currency: SupportedCurrency): string {
  return currency === 'sol'
    ? `${lamportsToSol(amount)} SOL`
    : `${formatTokenAmount(amount, 6)} ${currencySymbol(currency)}`
}

export interface Product {
  id: string
  /** The shop this product is sold from. */
  shop_id: string
  name: string
  /** What is in it, the size, the allergens — never sent with a payment. */
  details: string
  /** The heading the menu lists it under, empty for no category. */
  category: string
  /**
   * The canonical price, in the shop's currency's base units — every receipt
   * line and cart total is built from it. The name predates the currency
   * choice; the unit is always the shop's, never assumed to be SOL.
   */
  price_lamports: number
  created_at_unix: number
}

/** A shop with its catalogue — what the shop page and the payment page read. */
export interface ShopWithProducts extends Shop {
  products: Product[]
}

export interface ProductInput {
  name: string
  details: string
  category: string
  price_lamports: number
}

/** One heading's worth of a menu, in the order the products keep. */
export interface MenuGroup {
  /** Empty for products that have no category. */
  category: string
  products: Product[]
}

/**
 * A catalogue as a menu: products grouped under their category, headings in the
 * order they first appear. Uncategorised products keep their place rather than
 * being swept into a group at the end.
 */
export function menuGroups(products: Product[]): MenuGroup[] {
  const groups: MenuGroup[] = []
  for (const product of products) {
    const found = groups.find(group => group.category === product.category)
    if (found) found.products.push(product)
    else groups.push({ category: product.category, products: [product] })
  }
  return groups
}

/** One product and how many of it the seller is charging for. */
export interface CartLine {
  product: Product
  quantity: number
}

/** Totals the cart, in the unit the receipt is built from. */
export function cartTotal(lines: CartLine[]): number {
  return lines.reduce((total, line) => total + line.product.price_lamports * line.quantity, 0)
}

/** Thin wrapper around the API, in the same shape as `usePaymentApi`. */
export function useShopApi() {
  const config = useRuntimeConfig()
  const base = config.public.apiBase

  /** Every shop the account keeps, each with its catalogue. */
  async function list(): Promise<ShopWithProducts[]> {
    return await $fetch(`${base}/api/shops`, { headers: authHeaders() })
  }

  async function get(id: string): Promise<ShopWithProducts> {
    return await $fetch(`${base}/api/shops/${id}`, { headers: authHeaders() })
  }

  async function createShop(
    body: { name: string, description: string, currency?: SupportedCurrency }
  ): Promise<Shop> {
    return await $fetch(`${base}/api/shops`, {
      method: 'POST',
      body,
      headers: authHeaders()
    })
  }

  async function updateShop(
    id: string,
    body: { name: string, description: string, currency?: SupportedCurrency }
  ): Promise<Shop> {
    return await $fetch(`${base}/api/shops/${id}`, {
      method: 'PATCH',
      body,
      headers: authHeaders()
    })
  }

  async function createProduct(shopId: string, body: ProductInput): Promise<Product> {
    return await $fetch(`${base}/api/shops/${shopId}/products`, {
      method: 'POST',
      body,
      headers: authHeaders()
    })
  }

  async function updateProduct(
    shopId: string,
    productId: string,
    body: ProductInput
  ): Promise<Product> {
    return await $fetch(`${base}/api/shops/${shopId}/products/${productId}`, {
      method: 'PATCH',
      body,
      headers: authHeaders()
    })
  }

  async function deleteProduct(shopId: string, productId: string): Promise<{ ok: boolean }> {
    return await $fetch(`${base}/api/shops/${shopId}/products/${productId}`, {
      method: 'DELETE',
      headers: authHeaders()
    })
  }

  /** The headings this shop has used, for the editor's suggestions. */
  async function categories(shopId: string): Promise<string[]> {
    const answer: { categories: string[] } = await $fetch(`${base}/api/shops/${shopId}/categories`, {
      headers: authHeaders()
    })
    return answer.categories
  }

  /** Remembers a heading the seller typed, whether or not a product used it. */
  async function addCategory(shopId: string, name: string): Promise<string[]> {
    const answer: { categories: string[] } = await $fetch(`${base}/api/shops/${shopId}/categories`, {
      method: 'POST',
      body: { name },
      headers: authHeaders()
    })
    return answer.categories
  }

  /** Forgets a heading; products keep the text they were saved with. */
  async function deleteCategory(shopId: string, name: string): Promise<string[]> {
    const answer: { categories: string[] } = await $fetch(
      `${base}/api/shops/${shopId}/categories/${encodeURIComponent(name)}`,
      { method: 'DELETE', headers: authHeaders() }
    )
    return answer.categories
  }

  return {
    list,
    get,
    createShop,
    updateShop,
    createProduct,
    updateProduct,
    deleteProduct,
    categories,
    addCategory,
    deleteCategory
  }
}
