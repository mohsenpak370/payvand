export interface AdminStats {
  users: number
  active_sessions: number
  discount_codes: number
  pending_resets: number
}

export interface AdminUser {
  id: string
  email: string
  wallet: string | null
  created_at: number
  sessions: number
  codes: number
}

export interface UserPage {
  items: AdminUser[]
  total: number
  limit: number
  offset: number
}

export interface AdminPayment {
  id: string
  seller: string
  amount_lamports: number
  memo: string
  status: 'pending' | 'settled' | 'missing'
  settled_by: string | null
  created_at_unix: number
  settled_at_unix: number
}

export interface PaymentsPage {
  items: AdminPayment[]
  total: number
  cluster: string
  endpoint: string
}

/** One seller who asked to be told when Payvand can take real money. */
export interface WaitlistEntry {
  email: string
  created_at_unix: number
}

export interface WaitlistPage {
  items: WaitlistEntry[]
  total: number
}

/** One shop in the admin's records table. */
export interface AdminShop {
  id: string
  owner: string
  name: string
  description: string
  currency: string
  products: number
  created_at: number
}

/** One product in the admin's records table. */
export interface AdminProduct {
  id: string
  shop: string
  owner: string
  name: string
  details: string
  category: string
  price_lamports: number
  created_at: number
}

/** One discount code in the admin's records table. */
export interface AdminCode {
  id: string
  code: string
  percent_off: number
  owner: string
  created_at: number
}

/** One page of any record table — they share a shape so the UI can page all. */
export interface RecordPage<T> {
  items: T[]
  total: number
  limit: number
  offset: number
}

/** The body every admin write returns. */
export interface OkResponse {
  ok: true
}

/**
 * Admin endpoints. Access is decided by the API (`ADMIN_EMAILS`), not by
 * the client — every call can still come back 403 and the page must handle
 * that rather than assuming anything.
 */
export function useAdminApi() {
  const config = useRuntimeConfig()
  const base = config.public.apiBase

  async function fetchStats(): Promise<AdminStats> {
    return await $fetch<AdminStats>(`${base}/api/admin/stats`, {
      headers: authHeaders()
    })
  }

  async function fetchUsers(
    search: string | undefined,
    limit: number,
    offset: number
  ): Promise<UserPage> {
    return await $fetch<UserPage>(`${base}/api/admin/users`, {
      headers: authHeaders(),
      query: { ...(search ? { search } : {}), limit, offset }
    })
  }

  /** Live invoices, read from the cluster — may fail when the RPC does. */
  async function fetchPayments(limit = 20): Promise<PaymentsPage> {
    return await $fetch<PaymentsPage>(`${base}/api/admin/payments`, {
      headers: authHeaders(),
      query: { limit }
    })
  }

  /** Addresses waiting for a live cluster, newest first. */
  async function fetchWaitlist(): Promise<WaitlistPage> {
    return await $fetch<WaitlistPage>(`${base}/api/admin/waitlist`, {
      headers: authHeaders()
    })
  }

  async function removeWaitlistEntry(email: string): Promise<OkResponse> {
    // The address is a path segment, so it is encoded rather than trusted.
    return await $fetch<OkResponse>(`${base}/api/admin/waitlist/${encodeURIComponent(email)}`, {
      method: 'DELETE',
      headers: authHeaders()
    })
  }

  async function deleteUser(id: string): Promise<OkResponse> {
    return await $fetch<OkResponse>(`${base}/api/admin/users/${encodeURIComponent(id)}`, {
      method: 'DELETE',
      headers: authHeaders()
    })
  }

  /** Shops, searched by name or owner. */
  async function fetchShops(search: string, limit: number, offset: number) {
    return await $fetch<RecordPage<AdminShop>>(`${base}/api/admin/shops`, {
      headers: authHeaders(),
      query: { ...(search ? { search } : {}), limit, offset }
    })
  }

  /** Products, searched by name, shop or owner. */
  async function fetchProducts(search: string, limit: number, offset: number) {
    return await $fetch<RecordPage<AdminProduct>>(`${base}/api/admin/products`, {
      headers: authHeaders(),
      query: { ...(search ? { search } : {}), limit, offset }
    })
  }

  /** Discount codes, searched by code or owner. */
  async function fetchCodes(search: string, limit: number, offset: number) {
    return await $fetch<RecordPage<AdminCode>>(`${base}/api/admin/discounts`, {
      headers: authHeaders(),
      query: { ...(search ? { search } : {}), limit, offset }
    })
  }

  async function updateShop(
    id: string,
    body: { name: string, description?: string }
  ): Promise<OkResponse> {
    return await $fetch<OkResponse>(`${base}/api/admin/shops/${encodeURIComponent(id)}`, {
      method: 'PATCH',
      headers: authHeaders(),
      body
    })
  }

  async function deleteShop(id: string): Promise<OkResponse> {
    return await $fetch<OkResponse>(`${base}/api/admin/shops/${encodeURIComponent(id)}`, {
      method: 'DELETE',
      headers: authHeaders()
    })
  }

  async function updateProduct(
    id: string,
    body: { name: string, details?: string, category?: string, price_lamports: number }
  ): Promise<OkResponse> {
    return await $fetch<OkResponse>(`${base}/api/admin/products/${encodeURIComponent(id)}`, {
      method: 'PATCH',
      headers: authHeaders(),
      body
    })
  }

  async function deleteProduct(id: string): Promise<OkResponse> {
    return await $fetch<OkResponse>(`${base}/api/admin/products/${encodeURIComponent(id)}`, {
      method: 'DELETE',
      headers: authHeaders()
    })
  }

  async function deleteCode(id: string): Promise<OkResponse> {
    return await $fetch<OkResponse>(`${base}/api/admin/discounts/${encodeURIComponent(id)}`, {
      method: 'DELETE',
      headers: authHeaders()
    })
  }

  return {
    fetchStats,
    fetchUsers,
    fetchShops,
    fetchProducts,
    fetchCodes,
    updateShop,
    deleteShop,
    updateProduct,
    deleteProduct,
    deleteCode,
    fetchPayments,
    fetchWaitlist,
    removeWaitlistEntry,
    deleteUser
  }
}
