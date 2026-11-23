/**
 * The looks a payment page can wear.
 *
 * A seller picks one when the code is made, and it travels inside the invoice
 * payload, so the buyer's page arrives in the seller's colours without an
 * account or a lookup. The list is an allowlist: the payload is data from a QR
 * code, so it can name anything, and anything it names that is not one of these
 * falls back to the plain card.
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
 * default border never competes with the tinted one. Every variant prints as
 * the plain elevated card, because paper has no gradients to give.
 */
/**
 * The same palette at swatch size, for the seller's picker. One mapping for
 * both the card and its preview, so a design cannot look like one thing in the
 * picker and another on the page.
 */
export function cardDesignSwatch(value: unknown): string {
  switch (cardDesignOf(value)) {
    case 'aurora':
      return 'bg-gradient-to-br from-primary/45 to-primary/15'
    case 'sunset':
      return 'bg-gradient-to-br from-warning/55 to-error/30'
    case 'mint':
      return 'bg-gradient-to-br from-success/50 to-success/15'
    case 'ocean':
      return 'bg-gradient-to-br from-sky-400/50 to-primary/25'
    default:
      return 'border border-default bg-default'
  }
}

export function cardDesignClass(value: unknown): string {
  switch (cardDesignOf(value)) {
    case 'aurora':
      return 'border-primary/25 bg-gradient-to-br from-primary/10 via-default to-primary/5 print:bg-none print:bg-elevated print:border-default'
    case 'sunset':
      return 'border-warning/30 bg-gradient-to-br from-warning/15 via-default to-error/10 print:bg-none print:bg-elevated print:border-default'
    case 'mint':
      return 'border-success/30 bg-gradient-to-br from-success/12 via-default to-success/5 print:bg-none print:bg-elevated print:border-default'
    case 'ocean':
      return 'border-sky-400/30 bg-gradient-to-br from-sky-400/12 via-default to-primary/10 print:bg-none print:bg-elevated print:border-default'
    default:
      return 'border-default bg-default'
  }
}
