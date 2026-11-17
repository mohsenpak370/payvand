/**
 * How far a press may slide off the button and still count as the same press,
 * in CSS pixels: about a finger's width. A touch does not land on a pixel, and
 * a press that gives up on the first millimetre of drift would be unusable.
 *
 * But a press that has been dragged away is a buyer changing their mind, and it
 * must not pay. So this is the one number that separates the two, and it lives
 * here — out of the component, with `holdSlop.test.ts` holding both of its
 * sides — so that a future tweak has to say what it does to a drag-away.
 */
export const HOLD_SLOP_PX = 32

/** The box the press started on, as `getBoundingClientRect` reports it. */
export interface HoldBounds {
  left: number
  right: number
  top: number
  bottom: number
}

/** A pointer position, in the client coordinates a pointer event carries. */
export interface HoldPoint {
  x: number
  y: number
}

/**
 * Whether the pointer has left the button for good: outside the box grown by
 * `slop` on every side. The boundary is inside, so a drag of exactly the slop
 * keeps the hold; the pixel past it ends it. Pulling away diagonally is past on
 * both axes at once, which is the same answer.
 */
export function strayedFromHoldBounds(
  bounds: HoldBounds,
  point: HoldPoint,
  slop: number = HOLD_SLOP_PX
): boolean {
  return (
    point.x < bounds.left - slop
    || point.x > bounds.right + slop
    || point.y < bounds.top - slop
    || point.y > bounds.bottom + slop
  )
}

/** The parts of a `pointerdown` that decide whether it may start a hold. */
export interface HoldPress {
  /** 0 is the primary button; 1 and 2 are the middle and secondary ones. */
  button: number
  /** False for every touch after the first one down. */
  isPrimary: boolean
}

/**
 * Whether a `pointerdown` is the press a hold is for. Only the primary press
 * may start one: a right-click, a middle-click or a second finger would start a
 * countdown the buyer never meant, and the countdown pays.
 */
export function acceptsHoldPress(press: HoldPress): boolean {
  return press.button === 0 && press.isPrimary
}
