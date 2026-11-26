/**
 * The looks a payment page can wear.
 *
 * A seller picks one when they open the page for the customer, and it rides
 * inside the invoice payload, so the buyer's page arrives in the seller's
 * colours without an account or a lookup. The list is an allowlist: the payload
 * is data from a QR code, so it can name anything, and anything it names that
 * is not one of these falls back to the plain card.
 *
 * The colour wash and the pattern live in CSS (`app/assets/css/main.css`)
 * rather than in utility classes: a design is several background layers with
 * their own sizes, which a class list cannot say readably, and both the card and
 * its preview read the same rules — so a design cannot look like one thing in
 * the picker and another on the page.
 */

export const CARD_DESIGNS = ['classic', 'aurora', 'sunset', 'mint', 'ocean'] as const

export type CardDesign = (typeof CARD_DESIGNS)[number]

export function isCardDesign(value: unknown): value is CardDesign {
  return typeof value === 'string' && (CARD_DESIGNS as readonly string[]).includes(value)
}

/** The design a payload names, or the classic card when it names none we know. */
export function cardDesignOf(value: unknown): CardDesign {
  return isCardDesign(value) ? value : 'classic'
}

/**
 * The card's surface for a design: border and background together, so the
 * default border never competes with the tinted one. `classic` is the plain
 * elevated card; every other design names the class that draws its wash and
 * pattern, and prints as the plain card.
 */
export function cardDesignClass(value: unknown): string {
  const design = cardDesignOf(value)
  return design === 'classic' ? 'border-default bg-default' : `card-design card-design-${design}`
}

/** The same design at swatch size, for the picker's previews. */
export function cardDesignSwatch(value: unknown): string {
  return cardDesignClass(value)
}
