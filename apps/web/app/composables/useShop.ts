/**
 * The seller's shop and its product catalogue.
 *
 * Nothing here is on chain. A shop is the menu the payment page prices an
 * invoice from, and the receipt it produces travels inside the QR code — the
 * API still keeps no invoices, so a product is only a starting price the
 * seller can pick instead of typing.
 */

export interface Shop {
  name: string
  description: string
}

export interface Product {
  id: string
  name: string
  /** What is in it, the size, the allergens — never sent with a payment. */
  details: string
  /** The canonical price: every receipt line is built from it. */
  price_lamports: number
  created_at_unix: number
}

export interface ShopResponse {
  /** Null until the seller names their shop. */
  shop: Shop | null
  products: Product[]
}

export interface ProductInput {
  name: string
  details: string
  price_lamports: number
}

/** One product and how many of it the seller is charging for. */
export interface CartLine {
  product: Product
  quantity: number
}

/** Totals the cart, in the unit the receipt is built from. */
export function cartTotalLamports(lines: CartLine[]): number {
  return lines.reduce((total, line) => total + line.product.price_lamports * line.quantity, 0)
}

/** Thin wrapper around the API, in the same shape as `usePaymentApi`. */
export function useShopApi() {
  const config = useRuntimeConfig()
  const base = config.public.apiBase
  const { token } = useAuth()

  function authHeaders(): Record<string, string> {
    return token.value ? { authorization: `Bearer ${token.value}` } : {}
  }

  async function get(): Promise<ShopResponse> {
    return await $fetch(`${base}/api/shop`, { headers: authHeaders() })
  }

  /** Creates the shop, or renames the one this account already has. */
  async function saveShop(body: Shop): Promise<Shop> {
    return await $fetch(`${base}/api/shop`, {
      method: 'PUT',
      body,
      headers: authHeaders()
    })
  }

  async function createProduct(body: ProductInput): Promise<Product> {
    return await $fetch(`${base}/api/shop/products`, {
      method: 'POST',
      body,
      headers: authHeaders()
    })
  }

  async function updateProduct(id: string, body: ProductInput): Promise<Product> {
    return await $fetch(`${base}/api/shop/products/${id}`, {
      method: 'PATCH',
      body,
      headers: authHeaders()
    })
  }

  async function deleteProduct(id: string): Promise<{ ok: boolean }> {
    return await $fetch(`${base}/api/shop/products/${id}`, {
      method: 'DELETE',
      headers: authHeaders()
    })
  }

  return { get, saveShop, createProduct, updateProduct, deleteProduct }
}
