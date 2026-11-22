/**
 * The seller's shops and their product catalogues.
 *
 * Nothing here is on chain. A shop is a menu the payment page prices an invoice
 * from, and the receipt it produces travels inside the QR code — the API still
 * keeps no invoices, so a product is only a starting price the seller can pick
 * instead of typing.
 */

export interface Shop {
  id: string
  name: string
  description: string
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
  /** The canonical price: every receipt line is built from it. */
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

  /** Every shop the account keeps, each with its catalogue. */
  async function list(): Promise<ShopWithProducts[]> {
    return await $fetch(`${base}/api/shops`, { headers: authHeaders() })
  }

  async function get(id: string): Promise<ShopWithProducts> {
    return await $fetch(`${base}/api/shops/${id}`, { headers: authHeaders() })
  }

  async function createShop(body: { name: string, description: string }): Promise<Shop> {
    return await $fetch(`${base}/api/shops`, {
      method: 'POST',
      body,
      headers: authHeaders()
    })
  }

  async function updateShop(
    id: string,
    body: { name: string, description: string }
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

  return { list, get, createShop, updateShop, createProduct, updateProduct, deleteProduct }
}
