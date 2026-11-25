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

const rentCache = new Map<string, { readAt: number, lamports: number }>()

/**
 * A size is only answerable for the cluster it was asked of: rent parameters are
 * per cluster, and a quote for an invoice on another one must not inherit this
 * one's minimums. The key carries the cluster; an empty one is "whatever the
 * client is pointed at", which is how the cluster-less callers read.
 */
function cacheKey(cluster: string, space: number): string {
  return `${cluster}:${space}`
}

/** `getMinimumBalanceForRentExemption` for `space`, answered from the shared cache. */
export async function rentMinimum(
  client: ReturnType<typeof useSolanaClient>['client'],
  space: number,
  cluster = ''
): Promise<number> {
  const cached = rentCache.get(cacheKey(cluster, space))
  if (cached && Date.now() - cached.readAt < CACHE_TTL_MS) return cached.lamports

  const lamports = Number(
    await client.rpc.getMinimumBalanceForRentExemption(BigInt(space)).send()
  )
  rentCache.set(cacheKey(cluster, space), { readAt: Date.now(), lamports })
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
  rent: Record<string, number>,
  cluster = ''
): void {
  const readAt = Date.now()
  for (const [name, space] of Object.entries(spaces)) {
    const lamports = rent[name]
    // Zero is not a rent: every account costs something, and an answer that says
    // otherwise is not one a quote may use.
    if (typeof space !== 'number' || typeof lamports !== 'number' || lamports <= 0) continue
    rentCache.set(cacheKey(cluster, space), { readAt, lamports })
  }
}

/** Drops the cached rents, so the next quote asks the cluster again. */
export function clearRentCache(): void {
  rentCache.clear()
}
