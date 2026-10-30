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

/**
 * Admin endpoints. Access is decided by the API (`ADMIN_EMAILS`), not by
 * the client — every call can still come back 403 and the page must handle
 * that rather than assuming anything.
 */
export function useAdminApi() {
  const config = useRuntimeConfig()
  const base = config.public.apiBase
  const { token } = useAuth()

  function headers(): Record<string, string> {
    return token.value ? { authorization: `Bearer ${token.value}` } : {}
  }

  async function fetchStats(): Promise<AdminStats> {
    return await $fetch<AdminStats>(`${base}/api/admin/stats`, {
      headers: headers()
    })
  }

  async function fetchUsers(
    search: string | undefined,
    limit: number,
    offset: number
  ): Promise<UserPage> {
    return await $fetch<UserPage>(`${base}/api/admin/users`, {
      headers: headers(),
      query: { ...(search ? { search } : {}), limit, offset }
    })
  }

  /** Live invoices, read from the cluster — may fail when the RPC does. */
  async function fetchPayments(limit = 20): Promise<PaymentsPage> {
    return await $fetch<PaymentsPage>(`${base}/api/admin/payments`, {
      headers: headers(),
      query: { limit }
    })
  }

  async function deleteUser(id: string): Promise<void> {
    await $fetch(`${base}/api/admin/users/${id}`, {
      method: 'DELETE',
      headers: headers()
    })
  }

  return { fetchStats, fetchUsers, fetchPayments, deleteUser }
}
