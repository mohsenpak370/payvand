/**
 * Whether the signed-in account may open `/admin`.
 *
 * Authorization stays on the API: `ADMIN_EMAILS` is server configuration and
 * every admin route re-checks it, so the client only *asks* — one
 * `GET /api/admin/stats` per account, remembered in state — and never decides
 * for itself. A non-admin account is answered with 403, which becomes "no
 * link"; the admin page still handles 403 on its own, because hiding a menu
 * item is not authorization.
 */
export function useAdminAccess() {
  const config = useRuntimeConfig()
  const base = config.public.apiBase
  const { user, token } = useAuth()

  /** The verdict for one account id — a sign-out must not inherit it. */
  const verdict = useState<{ id: string, admin: boolean } | null>(
    'auth:admin',
    () => null
  )

  let inflight: Promise<boolean> | null = null

  const isAdmin = computed(
    () => verdict.value !== null
      && verdict.value.id === user.value?.id
      && verdict.value.admin
  )

  async function check(): Promise<boolean> {
    if (import.meta.server) return false
    const account = user.value
    const session = token.value
    if (!account || !session) return false
    if (verdict.value?.id === account.id) return verdict.value.admin
    if (inflight) return inflight

    inflight = (async () => {
      // A 200 means the API recognises the account; 403 is its "no", and
      // anything else (API restarting, a proxy gap) is unknown — the quiet
      // menu is the safe answer either way. The header never waits on this:
      // an unreachable API answers "no link", not a spinner over every
      // signed-in visit.
      const admin = await $fetch(`${base}/api/admin/stats`, {
        headers: { authorization: `Bearer ${session}` },
        signal: AbortSignal.timeout(5000)
      }).then(() => true, () => false)
      verdict.value = { id: account.id, admin }
      return admin
    })()

    const result = await inflight
    inflight = null
    return result
  }

  // The session plugin resolves the account after hydration, so watching the
  // id covers a restored session, a fresh sign-in and an account switch.
  watch(
    () => user.value?.id ?? null,
    (id) => {
      if (id) void check()
    },
    { immediate: true }
  )

  return { isAdmin, check }
}
