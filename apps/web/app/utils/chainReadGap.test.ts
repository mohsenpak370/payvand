import { describe, expect, it } from 'vitest'
import { CHAIN_READ_GAPS_MS, chainReadGapAfter } from './chainReadGap'

/**
 * The seconds the schedule waits after each read, spelled out rather than taken
 * from the helper: the point of these cases is the schedule itself. One poll
 * tick, then the gap doubles, until it is a minute and stays there.
 */
const SECONDS = [5, 10, 20, 60]

const gap = (reads: number) => chainReadGapAfter(reads)

describe('chainReadGapAfter', () => {
  it('reads the chain one poll tick after the page opens', () => {
    expect(gap(0)).toBe(5_000)
  })

  it('doubles the wait after each read that found nothing', () => {
    expect(gap(1)).toBe(10_000)
    expect(gap(2)).toBe(20_000)
  })

  it('holds the wait at a minute however long the page stays open', () => {
    expect(gap(3)).toBe(60_000)
    expect(gap(4)).toBe(60_000)
    expect(gap(50)).toBe(60_000)
    expect(gap(10_000)).toBe(60_000)
  })

  it('keeps its four steps, in order, all of them at least a tick', () => {
    expect([...CHAIN_READ_GAPS_MS]).toEqual(SECONDS.map(seconds => seconds * 1_000))
    for (let reads = 0; reads < 200; reads += 1) {
      expect(gap(reads)).toBeGreaterThanOrEqual(5_000)
      expect(gap(reads + 1)).toBeGreaterThanOrEqual(gap(reads))
    }
  })

  it('takes a count it cannot trust without reading any faster', () => {
    // A counter that never advanced is no reads yet; a fraction is whole reads.
    expect(gap(-1)).toBe(5_000)
    expect(gap(2.7)).toBe(20_000)
    // A caller that has lost the count waits the longest, never the shortest.
    expect(gap(Number.NaN)).toBe(60_000)
    expect(gap(Number.POSITIVE_INFINITY)).toBe(60_000)
  })
})
