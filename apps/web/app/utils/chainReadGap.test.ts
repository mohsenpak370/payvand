import { describe, expect, it } from 'vitest'
import {
  CHAIN_READ_GAPS_MS,
  CHAIN_READ_WINDOW_MS,
  chainReadGapAfter,
  chainReadWanted
} from './chainReadGap'

/**
 * The seconds the schedule waits after each read, spelled out rather than taken
 * from the helper: the point of these cases is the schedule itself. One poll
 * tick, then the gap doubles, inside a window of one minute.
 */
const SECONDS = [5, 10, 20]

const gap = (reads: number) => chainReadGapAfter(reads)

describe('chainReadGapAfter', () => {
  it('reads the chain one poll tick after the page opens', () => {
    expect(gap(0)).toBe(5_000)
  })

  it('doubles the wait after each read that found nothing', () => {
    expect(gap(1)).toBe(10_000)
    expect(gap(2)).toBe(20_000)
  })

  it('holds the wait at its last step', () => {
    expect(gap(3)).toBe(20_000)
    expect(gap(50)).toBe(20_000)
    expect(gap(10_000)).toBe(20_000)
  })

  it('keeps its three steps, in order, all of them at least a tick', () => {
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
    expect(gap(Number.NaN)).toBe(20_000)
    expect(gap(Number.POSITIVE_INFINITY)).toBe(20_000)
  })
})

describe('chainReadWanted', () => {
  it('reads through the first minute after a load or a send', () => {
    expect(chainReadWanted(0)).toBe(true)
    expect(chainReadWanted(5_000)).toBe(true)
    expect(chainReadWanted(55_000)).toBe(true)
    expect(chainReadWanted(CHAIN_READ_WINDOW_MS - 1)).toBe(true)
  })

  it('stops when the minute is up, leaving the page to the API poll', () => {
    expect(chainReadWanted(CHAIN_READ_WINDOW_MS)).toBe(false)
    expect(chainReadWanted(61_000)).toBe(false)
    expect(chainReadWanted(3_600_000)).toBe(false)
  })

  it('does not keep reading on a clock it cannot trust', () => {
    expect(chainReadWanted(Number.NaN)).toBe(false)
    expect(chainReadWanted(Number.POSITIVE_INFINITY)).toBe(false)
  })

  it('spends four reads on that minute, not twelve', () => {
    // Reads at 5s, 15s, 35s and 55s: the gaps accumulate inside the window and
    // the fifth would land at 75s, where the API poll has the page.
    let at = 0
    let reads = 0
    while (chainReadWanted(at + gap(reads))) {
      at += gap(reads)
      reads += 1
    }
    expect(reads).toBe(4)
    expect(at).toBe(55_000)
  })
})
