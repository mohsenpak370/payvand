import type { AdminStats, AdminUser } from './useAdminApi'

export const ADMIN_PAGE_SIZE = 20

/** The SQL half of the dashboard: stats, the paged account table and deletion. */
export function useAdminUsers() {
  const { t } = useI18n()
  const { fetchStats, fetchUsers, deleteUser } = useAdminApi()

  const stats = shallowRef<AdminStats | null>(null)
  const rows = ref<AdminUser[]>([])
  const total = shallowRef(0)
  const offset = shallowRef(0)
  const search = shallowRef('')
  const usersLoading = shallowRef(true)
  const usersFailure = shallowRef('')
  const usersBusy = shallowRef(false)
  const forbidden = shallowRef(false)

  /** ofetch surfaces the HTTP status in one of several shapes. */
  function statusOf(error: unknown): number | undefined {
    if (typeof error !== 'object' || error === null) return undefined
    const candidate = error as {
      status?: number
      statusCode?: number
      response?: { status?: number }
    }
    return candidate.status ?? candidate.statusCode ?? candidate.response?.status
  }

  async function loadUsers() {
    usersFailure.value = ''
    usersLoading.value = true
    try {
      stats.value = await fetchStats()
      const page = await fetchUsers(
        search.value.trim() || undefined,
        ADMIN_PAGE_SIZE,
        offset.value
      )
      rows.value = page.items
      total.value = page.total
      forbidden.value = false
    } catch (error) {
      if (statusOf(error) === 403) {
        forbidden.value = true
      } else {
        usersFailure.value = apiErrorMessage(error, t('admin.users.loadFailure'))
      }
    } finally {
      usersLoading.value = false
    }
  }

  async function searchUsers(query: string) {
    search.value = query
    offset.value = 0
    await loadUsers()
  }

  function pageDown() {
    offset.value = Math.max(0, offset.value - ADMIN_PAGE_SIZE)
    void loadUsers()
  }

  function pageUp() {
    offset.value += ADMIN_PAGE_SIZE
    void loadUsers()
  }

  async function removeUser(row: AdminUser) {
    const confirmed = window.confirm(t('admin.users.deleteConfirm', { email: row.email }))
    if (!confirmed) return

    usersBusy.value = true
    try {
      await deleteUser(row.id)
      // Stepping back a page keeps the table from going empty after the
      // last row on a later page is deleted.
      if (rows.value.length === 1 && offset.value > 0) {
        offset.value = Math.max(0, offset.value - ADMIN_PAGE_SIZE)
      }
      await loadUsers()
    } catch (error) {
      usersFailure.value = apiErrorMessage(error, t('admin.users.deleteFailure'))
    } finally {
      usersBusy.value = false
    }
  }

  onMounted(loadUsers)

  return {
    stats,
    rows,
    total,
    pageSize: ADMIN_PAGE_SIZE,
    offset,
    usersLoading,
    usersFailure,
    usersBusy,
    forbidden,
    searchUsers,
    pageDown,
    pageUp,
    removeUser
  }
}
