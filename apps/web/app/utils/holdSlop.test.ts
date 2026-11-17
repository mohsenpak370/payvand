import { describe, expect, it } from 'vitest'
import { HOLD_SLOP_PX, acceptsHoldPress, strayedFromHoldBounds } from './holdSlop'

/**
 * A 100×40 button at (200, 300). The numbers are literals rather than derived
 * from the helper: the point of these cases is the threshold itself.
 */
const BUTTON = { left: 200, right: 300, top: 300, bottom: 340 }

/** 32: a finger's width, and the rule that decides drag-away from press. */
const SLOP = HOLD_SLOP_PX

const strays = (x: number, y: number) => strayedFromHoldBounds(BUTTON, { x, y })

describe('strayedFromHoldBounds', () => {
  it('keeps a press that has not moved, or is inside the button', () => {
    expect(strays(250, 320)).toBe(false)
    expect(strays(200, 300)).toBe(false)
    expect(strays(300, 340)).toBe(false)
  })

  it('keeps a press that drifted exactly the slop off each edge', () => {
    expect(strays(200 - SLOP, 320)).toBe(false)
    expect(strays(300 + SLOP, 320)).toBe(false)
    expect(strays(250, 300 - SLOP)).toBe(false)
    expect(strays(250, 340 + SLOP)).toBe(false)
  })

  it('ends a press one pixel past the slop on each edge', () => {
    expect(strays(200 - SLOP - 1, 320)).toBe(true)
    expect(strays(300 + SLOP + 1, 320)).toBe(true)
    expect(strays(250, 300 - SLOP - 1)).toBe(true)
    expect(strays(250, 340 + SLOP + 1)).toBe(true)
  })

  it('treats a diagonal drag by whichever axis left first', () => {
    // Inside on both axes: still the same press.
    expect(strays(200 - SLOP, 340 + SLOP)).toBe(false)
    // One axis past is enough, even with the other still inside.
    expect(strays(200 - SLOP - 1, 320)).toBe(true)
    expect(strays(250, 340 + SLOP + 1)).toBe(true)
    // Both past: the same answer.
    expect(strays(200 - SLOP - 1, 340 + SLOP + 1)).toBe(true)
  })

  it('is the slop, not the button, that decides the boundary', () => {
    // A tight slop ends the press while the pointer is still on the button,
    // which is why the default is the finger's width and not a hair.
    expect(strayedFromHoldBounds(BUTTON, { x: 250, y: 320 }, 0)).toBe(false)
    expect(strayedFromHoldBounds(BUTTON, { x: 340, y: 320 }, 0)).toBe(true)
    expect(strayedFromHoldBounds(BUTTON, { x: 340, y: 320 }, 40)).toBe(false)
    expect(strayedFromHoldBounds(BUTTON, { x: 340, y: 320 }, 39)).toBe(true)
  })

  it('holds the threshold at 32 CSS pixels', () => {
    // A change here is a change to how a buyer cancels a payment, so it should
    // be made deliberately, with this line and the cases above in view.
    expect(SLOP).toBe(32)
  })
})

describe('acceptsHoldPress', () => {
  it('takes the primary press', () => {
    expect(acceptsHoldPress({ button: 0, isPrimary: true })).toBe(true)
  })

  it('refuses the presses that are not a decision to pay', () => {
    // A right-click, a middle-click, and every finger after the first one down:
    // each would otherwise start a countdown that ends in a charge.
    expect(acceptsHoldPress({ button: 2, isPrimary: true })).toBe(false)
    expect(acceptsHoldPress({ button: 1, isPrimary: true })).toBe(false)
    expect(acceptsHoldPress({ button: 0, isPrimary: false })).toBe(false)
  })
})
