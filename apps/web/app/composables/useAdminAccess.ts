/**
 * Whether the signed-in account may open `/admin`.
 *
 * Authorization stays on the API: `ADMIN_EMAILS` is server configuration and
 * every admin route re-checks it, so the client only *asks* — one successful
 * `GET /api/admin/stats` per account, remembered in state — and never decides
 * for itself. An explicit 403 is the API's "no" and becomes "no link"; a
 * timeout or a cold serverless function is unknown, so the check retries
 * instead of branding an admin as a non-admin for the rest of the visit.
 */

import { httpStatus } from '~/utils/httpStatus'

const ADMIN_CHECK_TIMEOUT_MS = 15_000
const ADMIN_RETRY_DELAYS_MS = [2_000, 5_000, 10_000]

type Retry = {
  id: string
  timer: ReturnType<typeof setTimeout>
  attempts: number
}

export function useAdminAccess() {
  const config = useRuntimeConfig()
  const base = config.public.apiBase
  const { user, token } = useAuth()

  /** The verdict for one account id — a sign-out must not inherit it. */
  const verdict = useState<{ id: string, admin: boolean } | null>(
    'auth:admin',
    () => null
  )

  let inflight: { id: string, promise: Promise<boolean> } | null = null
  let retry: Retry | null = null

  const isAdmin = computed(
    () => verdict.value !== null
      && verdict.value.id === user.value?.id
      && verdict.value.admin
  )

  function clearRetry(id?: string) {
    if (retry && (id === undefined || retry.id === id)) {
      clearTimeout(retry.timer)
      retry = null
    }
  }

  function scheduleRetry(id: string) {
    // `check` never fetches on the server, so anything that reaches here is a
    // browser visit whose API answer was inconclusive.
    if (retry?.id === id) return
    const attempts = retry?.id === id ? retry.attempts : 0
    clearRetry()
    const delay = ADMIN_RETRY_DELAYS_MS[Math.min(attempts, ADMIN_RETRY_DELAYS_MS.length - 1)]
    const timer = setTimeout(() => {
      retry = null
      if (user.value?.id === id && token.value) void check()
    }, delay)
    retry = { id, timer, attempts: attempts + 1 }
  }

  async function check(): Promise<boolean> {
    if (import.meta.server) return false
    const account = user.value
    const session = token.value
    if (!account || !session) return false
    if (verdict.value?.id === account.id) return verdict.value.admin
    if (inflight?.id === account.id) return inflight.promise

    const id = account.id
    const task = (async () => {
      try {
        await $fetch(`${base}/api/admin/stats`, {
          headers: { authorization: `Bearer ${session}` },
          signal: AbortSignal.timeout(ADMIN_CHECK_TIMEOUT_MS)
        })
        verdict.value = { id, admin: true }
        clearRetry(id)
        return true
      } catch (error) {
        if (httpStatus(error) === 403) {
          verdict.value = { id, admin: false }
          clearRetry(id)
          return false
        }
        // A timeout, a proxy gap or an expired session says nothing about
        // admin rights. Stay undecided and let the next attempt answer.
        scheduleRetry(id)
        return false
      }
    })()

    inflight = { id, promise: task }
    try {
      return await task
    } finally {
      if (inflight?.id === id) inflight = null
    }
  }

  // The session plugin resolves the account after hydration, so watching the
  // id covers a restored session, a fresh sign-in and an account switch.
  watch(
    () => user.value?.id ?? null,
    (id) => {
      clearRetry()
      if (id) void check()
    },
    { immediate: true }
  )

  return { isAdmin, check }
}
