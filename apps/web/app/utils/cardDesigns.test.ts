import { describe, expect, it } from 'vitest'
import { CARD_DESIGNS, cardDesignClass, cardDesignOf, isCardDesign } from './cardDesigns'

describe('cardDesignOf', () => {
  it('keeps a design the app knows', () => {
    expect(cardDesignOf('sunset')).toBe('sunset')
    expect(cardDesignOf('ocean')).toBe('ocean')
  })

  it('falls back to the classic card for anything else', () => {
    // The value comes from a QR code, so it can be any string at all.
    expect(cardDesignOf(undefined)).toBe('classic')
    expect(cardDesignOf('')).toBe('classic')
    expect(cardDesignOf('bg-red-500')).toBe('classic')
    expect(cardDesignOf(42)).toBe('classic')
  })
})

describe('cardDesignClass', () => {
  it('tints each design and keeps the plain card plain', () => {
    expect(cardDesignClass('classic')).toBe('border-default bg-default')
    for (const design of CARD_DESIGNS.filter(design => design !== 'classic')) {
      expect(cardDesignClass(design)).toContain('bg-gradient-to-br')
    }
  })

  it('prints every design as the plain card', () => {
    for (const design of CARD_DESIGNS) {
      const printed = cardDesignClass(design)
      if (design === 'classic') continue
      expect(printed).toContain('print:bg-none')
      expect(printed).toContain('print:bg-elevated')
    }
  })

  it('ignores a design it does not know', () => {
    expect(cardDesignClass('purple')).toBe(cardDesignClass('classic'))
    expect(isCardDesign('purple')).toBe(false)
  })
})
