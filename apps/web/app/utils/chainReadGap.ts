/**
 * How long the open checkout leaves between its own reads of the buyer's node.
 *
 * The record poll asks the API every five seconds, and the API reads the cluster
 * at `confirmed`, so the poll alone settles a page a slot or two after a payment.
 * What the poll cannot cover is the API's own lag — an indexer behind, a node
 * that has not voted the send `confirmed` yet, an invoice paid in another
 * currency or from another device — and the chain has that answer first. So the
 * page also reads the chain, on this schedule: one poll tick behind the first
 * poll, then each gap doubling until it is a minute.
 *
 * The doubling is the whole point. A page opened on an unpaid invoice can sit
 * there all afternoon, and it must not spend three `getAccountInfo` calls of the
 * buyer's RPC every five seconds for hours. The cap keeps a long-lived page
 * asking, at a cost of one read a minute, so a payment made elsewhere still
 * shows up on a tab nobody has touched.
 *
 * Only the schedule lives here — no clock, no page — so `chainReadGap.test.ts`
 * can hold every step of it, the way `holdSlop` keeps its threshold out of the
 * button. The composable counts the reads it has made and asks what follows.
 */
export const CHAIN_READ_GAPS_MS = [5_000, 10_000, 20_000, 60_000] as const

/**
 * The wait before the read that follows `reads` earlier ones, in milliseconds:
 * the first read goes out on the first poll tick, the second after two more, and
 * so on, never further apart than a minute.
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
