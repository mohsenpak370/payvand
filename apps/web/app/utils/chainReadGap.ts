/**
 * How long the open checkout keeps reading the buyer's node itself.
 *
 * The record poll asks the API every five seconds, and the API reads the cluster
 * at `confirmed`, so the poll alone settles a page a slot or two after a payment.
 * What the poll cannot cover is the API's own lag just after a load or just after
 * a send — a node that has not voted the send `confirmed` yet, an indexer behind,
 * an invoice paid in another currency or from another device — and the chain has
 * that answer first. So for the first minute after a load, a send or a currency
 * switch, the page also reads the chain, on the schedule below.
 *
 * Then it stops. A page opened on an unpaid invoice can sit there all afternoon,
 * and it must not spend the buyer's RPC quota for hours to re-learn what the API
 * polls already carry: after this window the page asks nobody but the API.
 */
export const CHAIN_READ_WINDOW_MS = 60_000

/**
 * The waits between the page's own reads, inside that window.
 *
 * The first read goes out on the first poll tick, the second after two more, and
 * so on — reads at 5s, 15s, 35s and 55s, which is the whole minute the API can
 * trail a fresh send, at four reads rather than twelve.
 *
 * The doubling is the whole point: most of that minute is covered by the later,
 * cheaper reads, and a payment that needs the chain's answer arrives in the
 * seconds right after the send, when the reads are closest together.
 *
 * Only the schedule lives here — no clock, no page — so `chainReadGap.test.ts`
 * can hold every step of it, the way `holdSlop` keeps its threshold out of the
 * button. The composable counts the reads it has made and asks what follows.
 */
export const CHAIN_READ_GAPS_MS = [5_000, 10_000, 20_000] as const

/**
 * The wait before the read that follows `reads` earlier ones, in milliseconds:
 * the first read goes out on the first poll tick, the second after two more, and
 * so on, never further apart than the last step.
 *
 * A count it cannot trust does not make the page read faster. A count that is
 * negative or fractional is read as "no reads yet" or as the whole reads in it,
 * and one that is not a number at all — the sign of a caller that has lost track
 * — takes the longest wait, because the mistake a slow schedule makes is a late
 * payment, and the mistake a fast one makes is a buyer's quota.
 */
export function chainReadGapAfter(reads: number): number {
  if (!Number.isFinite(reads)) return CHAIN_READ_GAPS_MS[CHAIN_READ_GAPS_MS.length - 1]!
  const index = Math.min(
    Math.max(0, Math.floor(reads)),
    CHAIN_READ_GAPS_MS.length - 1
  )
  return CHAIN_READ_GAPS_MS[index]!
}

/**
 * Whether the page is still inside its own-read window, `elapsedMs` after the
 * load, send or switch that started it.
 *
 * An elapsed time it cannot trust is not a reason to keep reading the chain, so
 * an unusable number ends the schedule: the poll still answers the page, and the
 * mistake a lost clock makes should not be a minute of extra RPC reads.
 */
export function chainReadWanted(elapsedMs: number): boolean {
  if (!Number.isFinite(elapsedMs)) return false
  return elapsedMs < CHAIN_READ_WINDOW_MS
}
