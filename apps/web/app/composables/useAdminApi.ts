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

  async function fetchUsers(search?: string): Promise<AdminUser[]> {
    return await $fetch<AdminUser[]>(`${base}/api/admin/users`, {
      headers: headers(),
      query: search ? { search } : {}
    })
  }

  async function deleteUser(id: string): Promise<void> {
    await $fetch(`${base}/api/admin/users/${id}`, {
      method: 'DELETE',
      headers: headers()
    })
  }

  return { fetchStats, fetchUsers, deleteUser }
}
