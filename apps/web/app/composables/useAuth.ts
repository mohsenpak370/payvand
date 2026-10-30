export interface AuthUser {
  id: string
  email: string
  wallet: string | null
  created_at: number
}

interface AuthResponse {
  user: AuthUser
  token: string
}

const TOKEN_KEY = 'payvand.token'
const USER_KEY = 'payvand.user'

/** Mirrors `MIN_PASSWORD_LEN` in the Axum API. */
export const MIN_PASSWORD_LENGTH = 8

/** Only allows same-origin style paths (`/sell`, not `//evil.com`). */
export function safeNextPath(value: unknown): string {
  if (typeof value !== 'string') return '/'
  if (!value.startsWith('/') || value.startsWith('//')) return '/'
  return value
}

/** Shared so concurrent calls to `ensureSession` share one round trip. */
let inflight: Promise<boolean> | null = null

/**
 * Email + password accounts backed by the Axum API.
 *
 * The session is an opaque bearer token in `localStorage`. `users.wallet` is
 * still null for everyone — it is the slot for adding Solana wallet sign-in
 * to the same account later.
 */
export function useAuth() {
  const config = useRuntimeConfig()
  const base = config.public.apiBase

  const user = useState<AuthUser | null>('auth:user', () => null)
  const token = useState<string | null>('auth:token', () => null)

  function headers(): Record<string, string> {
    return token.value ? { authorization: `Bearer ${token.value}` } : {}
  }

  function persist(next: AuthResponse) {
    token.value = next.token
    user.value = next.user
    if (import.meta.client) {
      localStorage.setItem(TOKEN_KEY, next.token)
      localStorage.setItem(USER_KEY, JSON.stringify(next.user))
    }
  }

  function clear() {
    token.value = null
    user.value = null
    if (import.meta.client) {
      localStorage.removeItem(TOKEN_KEY)
      localStorage.removeItem(USER_KEY)
    }
  }

  /** Reads the stored session without contacting the server. */
  function restore() {
    if (!import.meta.client || token.value) return
    const stored = localStorage.getItem(TOKEN_KEY)
    if (!stored) return
    token.value = stored
    const cached = localStorage.getItem(USER_KEY)
    if (cached) {
      try {
        user.value = JSON.parse(cached) as AuthUser
      } catch {
        user.value = null
      }
    }
  }

  async function signup(email: string, password: string): Promise<AuthUser> {
    const response = await $fetch<AuthResponse>(`${base}/api/auth/signup`, {
      method: 'POST',
      body: { email, password }
    })
    persist(response)
    return response.user
  }

  async function signin(email: string, password: string): Promise<AuthUser> {
    const response = await $fetch<AuthResponse>(`${base}/api/auth/signin`, {
      method: 'POST',
      body: { email, password }
    })
    persist(response)
    return response.user
  }

  async function signout() {
    try {
      await $fetch(`${base}/api/auth/signout`, {
        method: 'POST',
        headers: headers()
      })
    } catch {
      // The server may already have dropped the session; clear it anyway.
    }
    clear()
  }

  /**
   * Starts password recovery. The API answers the same generic message for
   * every address, so success only means "the request was accepted" — the
   * page must never claim that an email was definitely sent.
   */
  async function forgotPassword(email: string): Promise<string> {
    const response = await $fetch<{ ok: boolean, message: string }>(
      `${base}/api/auth/forgot-password`,
      { method: 'POST', body: { email } }
    )
    return response.message
  }

  /** Exchanges the emailed one-time token for a new password. */
  async function resetPassword(token: string, password: string): Promise<void> {
    await $fetch(`${base}/api/auth/reset-password`, {
      method: 'POST',
      body: { token, password }
    })
  }

  /**
   * Restores the stored session and checks it against the server. Resolves
   * `false` when there is no usable account, so route middleware can bounce
   * the visitor to /signin.
   */
  async function ensureSession(): Promise<boolean> {
    if (import.meta.server) return false
    if (user.value && token.value) return true
    if (inflight) return inflight

    inflight = (async () => {
      restore()
      if (!token.value) return false
      try {
        const me = await $fetch<AuthUser>(`${base}/api/auth/me`, {
          headers: headers()
        })
        user.value = me
        if (import.meta.client) {
          localStorage.setItem(USER_KEY, JSON.stringify(me))
        }
        return true
      } catch {
        clear()
        return false
      }
    })()

    const result = await inflight
    inflight = null
    return result
  }

  return { user, token, signup, signin, signout, forgotPassword, resetPassword, ensureSession, restore }
}

/**
 * Pulls the message out of an API failure. `$fetch` puts the response body in
 * `error.data` for non-2xx answers, and the Axum API always answers with
 * `{ "error": "..." }`. Anything else falls back to a generic message so a
 * network error never surfaces as raw internals.
 */
export function apiErrorMessage(error: unknown, fallback: string): string {
  if (typeof error === 'object' && error !== null && 'data' in error) {
    const data = (error as { data?: unknown }).data
    if (typeof data === 'object' && data !== null && 'error' in data) {
      const message = (data as { error?: unknown }).error
      if (typeof message === 'string' && message.length > 0) return message
    }
  }
  if (error instanceof Error && error.message && !error.message.startsWith('FetchError')) {
    return error.message
  }
  return fallback
}
