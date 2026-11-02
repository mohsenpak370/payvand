export interface AuthUser {
  id: string
  email: string
  wallet: string | null
  created_at: number
}

export interface WalletChallenge {
  message: string
  purpose: 'link' | 'signin'
  expires_at: number
}

export interface MyTransaction {
  id: string
  direction: 'sent' | 'received'
  counterparty: string
  amount_lamports: number
  memo: string
  status: 'pending' | 'settled' | 'missing'
  created_at_unix: number
  settled_at_unix: number
}

export interface TransactionsResponse {
  wallet: string
  items: MyTransaction[]
  total: number
  cluster: string
}

const BASE58_ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'

/** Solana's base58 — the encoding the API expects signatures in. */
export function base58Encode(bytes: Uint8Array): string {
  let zeros = 0
  while (zeros < bytes.length && bytes[zeros] === 0) zeros++
  let value = 0n
  for (const byte of bytes) value = (value << 8n) | BigInt(byte)
  let out = ''
  while (value > 0n) {
    out = BASE58_ALPHABET[Number(value % 58n)] + out
    value /= 58n
  }
  return '1'.repeat(zeros) + out
}

interface AuthResponse {
  user: AuthUser
  token: string
}

const TOKEN_KEY = 'payvand.token'
const USER_KEY = 'payvand.user'

/**
 * The session probe must never pin the router: the dev proxy holds
 * requests to a stopped API forever instead of failing fast, so a hung
 * probe would leave every protected page blank.
 */
const SESSION_CHECK_TIMEOUT_MS = 5000

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
 * The session is an opaque bearer token in `localStorage`. `users.wallet`
 * holds the Solana address linked from the profile page — once linked, the
 * same account can also be opened by signing a challenge with that wallet.
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

  /** Step 1 of wallet sign-in/link: the exact text the wallet must sign. */
  async function walletChallenge(
    address: string,
    purpose: 'link' | 'signin'
  ): Promise<WalletChallenge> {
    return await $fetch<WalletChallenge>(`${base}/api/auth/wallet/challenge`, {
      method: 'POST',
      body: { address, purpose }
    })
  }

  /** Attaches the signed wallet to the account that is signed in now. */
  async function linkWallet(address: string, signature: string): Promise<AuthUser> {
    const response = await $fetch<{ user: AuthUser }>(`${base}/api/auth/wallet/link`, {
      method: 'POST',
      headers: headers(),
      body: { address, signature }
    })
    user.value = response.user
    if (import.meta.client) {
      localStorage.setItem(USER_KEY, JSON.stringify(response.user))
    }
    return response.user
  }

  /** Detaches the wallet; email and password keep working. */
  async function unlinkWallet(): Promise<AuthUser> {
    const response = await $fetch<{ user: AuthUser }>(`${base}/api/auth/wallet/unlink`, {
      method: 'POST',
      headers: headers()
    })
    user.value = response.user
    if (import.meta.client) {
      localStorage.setItem(USER_KEY, JSON.stringify(response.user))
    }
    return response.user
  }

  /** Opens a session for the account the wallet is linked to. */
  async function signinWithWallet(address: string, signature: string): Promise<AuthUser> {
    const response = await $fetch<AuthResponse>(`${base}/api/auth/wallet/signin`, {
      method: 'POST',
      body: { address, signature }
    })
    persist(response)
    return response.user
  }

  /** This account's on-chain transactions, for its linked wallet. */
  async function myTransactions(): Promise<TransactionsResponse> {
    return await $fetch<TransactionsResponse>(`${base}/api/me/transactions`, {
      headers: headers()
    })
  }

  /**
   * True only when the server explicitly rejected the token. A network
   * failure (API restarting, dev proxy hiccup) says nothing about the
   * session and must never log the visitor out.
   */
  function isAuthRejection(error: unknown): boolean {
    if (typeof error !== 'object' || error === null) return false
    const { status, statusCode } = error as { status?: number, statusCode?: number }
    return status === 401 || statusCode === 401
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
          headers: headers(),
          signal: AbortSignal.timeout(SESSION_CHECK_TIMEOUT_MS)
        })
        user.value = me
        if (import.meta.client) {
          localStorage.setItem(USER_KEY, JSON.stringify(me))
        }
        return true
      } catch (error) {
        // Only an explicit 401 means the session is dead. Anything else is
        // a transport problem: keep the cached session so a flaky API
        // cannot flash the login page over a still-valid account.
        if (isAuthRejection(error)) {
          clear()
          return false
        }
        return Boolean(user.value || token.value)
      }
    })()

    const result = await inflight
    inflight = null
    return result
  }

  return {
    user,
    token,
    signup,
    signin,
    signout,
    forgotPassword,
    resetPassword,
    ensureSession,
    restore,
    walletChallenge,
    linkWallet,
    unlinkWallet,
    signinWithWallet,
    myTransactions
  }
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
