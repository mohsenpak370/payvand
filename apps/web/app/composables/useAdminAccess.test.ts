import { computed, effectScope, ref, watch, type Ref } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useAdminAccess } from './useAdminAccess'

/**
 * The header asks the API once per account, but a cold serverless function is
 * slower than the header's patience. That first timeout must not become a
 * permanent "not admin": the `/admin` page may still load moments later, and
 * the account menu has to catch up instead of leaving an admin without a link.
 */

let scope: ReturnType<typeof effectScope>
let states: Map<string, Ref<unknown>>
let user: Ref<{ id: string, email: string } | null>
let token: Ref<string | null>
let fetchStats: ReturnType<typeof vi.fn>

function account(id: string) {
  return { id, email: `${id}@example.com` }
}

/** Let the watcher, the fetch promise and any scheduled retry all settle. */
async function settle(ms = 0) {
  await vi.advanceTimersByTimeAsync(ms)
}

beforeEach(() => {
  vi.useFakeTimers()
  scope = effectScope()
  states = new Map()
  user = ref(null)
  token = ref(null)
  fetchStats = vi.fn()

  vi.stubGlobal('computed', computed)
  vi.stubGlobal('watch', watch)
  vi.stubGlobal('useRuntimeConfig', () => ({ public: { apiBase: 'https://api.test' } }))
  vi.stubGlobal('useAuth', () => ({ user, token }))
  vi.stubGlobal('useState', (key: string, init: () => unknown) => {
    if (!states.has(key)) states.set(key, ref(init()))
    return states.get(key)
  })
  vi.stubGlobal('$fetch', fetchStats)
})

afterEach(() => {
  scope.stop()
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

describe('useAdminAccess', () => {
  it('retries a transient stats failure instead of caching a false verdict', async () => {
    const access = scope.run(() => useAdminAccess())!
    fetchStats.mockRejectedValueOnce(new Error('timeout'))
    user.value = account('admin')
    token.value = 'token'
    await settle()

    expect(access.isAdmin.value).toBe(false)
    expect(states.get('auth:admin')?.value).toBeNull()

    fetchStats.mockResolvedValueOnce({ users: 1 })
    await settle(2000)

    expect(access.isAdmin.value).toBe(true)
    expect(states.get('auth:admin')?.value).toEqual({ id: 'admin', admin: true })
  })

  it('does not reuse one account\'s verdict for another', async () => {
    const access = scope.run(() => useAdminAccess())!
    fetchStats
      .mockResolvedValueOnce({ users: 1 })
      .mockRejectedValueOnce({ status: 403 })
      .mockResolvedValueOnce({ users: 1 })

    user.value = account('first')
    token.value = 'token'
    await settle()
    expect(access.isAdmin.value).toBe(true)

    user.value = account('second')
    await settle()
    expect(access.isAdmin.value).toBe(false)
    expect(fetchStats).toHaveBeenCalledTimes(2)

    user.value = account('first')
    await settle()
    expect(access.isAdmin.value).toBe(true)
    expect(fetchStats).toHaveBeenCalledTimes(3)
  })

  it('caches an explicit 403 and stops asking for that account', async () => {
    const access = scope.run(() => useAdminAccess())!
    fetchStats.mockRejectedValueOnce({ status: 403 })
    user.value = account('customer')
    token.value = 'token'
    await settle()

    expect(access.isAdmin.value).toBe(false)
    expect(states.get('auth:admin')?.value).toEqual({ id: 'customer', admin: false })

    await settle(30_000)
    expect(fetchStats).toHaveBeenCalledTimes(1)
  })
})
