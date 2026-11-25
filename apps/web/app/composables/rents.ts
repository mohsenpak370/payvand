/**
 * `getMinimumBalanceForRentExemption`, asked once per account size for every
 * quote on the page.
 *
 * The checkout quotes the invoice and the receipt, the allowance page quotes the
 * budget and the token budget, and the two overlap: a token payment asks for the
 * receipt from both. The answers move only when the cluster's rent parameters
 * change, so they are held for a minute at module level — a second composable
 * instance, a currency toggle, or a page visited again inside that minute asks
 * the cluster nothing.
 *
 * A per-instance cache is the bug this replaces: every `useBudget()` started with
 * an empty map, so a page that mounted two of them, or a second visit, asked the
 * same sizes all over again for numbers nothing had changed.
 */

/**
 * How long a rent answer stays fresh. The same minute the fee policy and the
 * account existence reads use, so a whole quote is at most a minute old.
 */
const CACHE_TTL_MS = 60_000

const rentCache = new Map<number, { readAt: number, lamports: number }>()

/** `getMinimumBalanceForRentExemption` for `space`, answered from the shared cache. */
export async function rentMinimum(
  client: ReturnType<typeof useSolanaClient>['client'],
  space: number
): Promise<number> {
  const cached = rentCache.get(space)
  if (cached && Date.now() - cached.readAt < CACHE_TTL_MS) return cached.lamports

  const lamports = Number(
    await client.rpc.getMinimumBalanceForRentExemption(BigInt(space)).send()
  )
  rentCache.set(space, { readAt: Date.now(), lamports })
  return lamports
}

/**
 * Seeds the cache from one answer that already carries every size a quote asks
 * for — `GET /api/config` returns the cluster's minimums beside the policy.
 * A quote served from it then reads the cluster no rent at all; a size the
 * answer did not name is still asked for on demand.
 */
export function primeRentCache(
  spaces: Record<string, number>,
  rent: Record<string, number>
): void {
  const readAt = Date.now()
  for (const [name, space] of Object.entries(spaces)) {
    const lamports = rent[name]
    if (typeof space !== 'number' || typeof lamports !== 'number') continue
    rentCache.set(space, { readAt, lamports })
  }
}

/** Drops the cached rents, so the next quote asks the cluster again. */
export function clearRentCache(): void {
  rentCache.clear()
}
