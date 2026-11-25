import { describe, expect, it } from 'vitest'
import { QR_MARGIN, clearMiddle, coverBox, readGrid } from '~/utils/qrPlate'

/**
 * The hero's mark sits in a clearing cut on the code's own grid.
 *
 * The generator (`createQR` in `@vue-solana/core`) draws `margin = 2`, one
 * rounded rect per dark module at 80% of its cell, and three finder patterns as
 * 7/5/3-cell rects. This suite builds exactly that markup, so the numbers here
 * are the ones the browser will see, and the property under test is the one the
 * eye cares about: no dot is left half covered by the plate.
 */

const SIZE = 280
const COUNT = 25
const CELL = (SIZE - QR_MARGIN * 2) / COUNT
const DOT = CELL * 0.8

/** A code drawn the way the generator draws one, with `dark` modules lit. */
function svgWith(dark: Array<[number, number]>): string {
  const parts = [`<svg width="${SIZE}" height="${SIZE}" viewBox="0 0 ${SIZE} ${SIZE}"><rect width="${SIZE}" height="${SIZE}" fill="white"/>`]
  for (const [row, col] of [[0, 0], [0, COUNT - 7], [COUNT - 7, 0]] as Array<[number, number]>) {
    const x = QR_MARGIN + col * CELL
    const y = QR_MARGIN + row * CELL
    parts.push(`<rect x="${x}" y="${y}" width="${7 * CELL}" height="${7 * CELL}" fill="black" rx="${CELL}" ry="${CELL}"/>`)
    parts.push(`<rect x="${x + CELL}" y="${y + CELL}" width="${5 * CELL}" height="${5 * CELL}" fill="white" rx="${CELL * 0.5}" ry="${CELL * 0.5}"/>`)
    parts.push(`<rect x="${x + 2 * CELL}" y="${y + 2 * CELL}" width="${3 * CELL}" height="${3 * CELL}" fill="black" rx="${CELL * 0.3}" ry="${CELL * 0.3}"/>`)
  }
  for (const [row, col] of dark) {
    const x = QR_MARGIN + col * CELL + (CELL - DOT) / 2
    const y = QR_MARGIN + row * CELL + (CELL - DOT) / 2
    parts.push(`<rect x="${x}" y="${y}" width="${DOT}" height="${DOT}" fill="black" rx="${DOT * 0.3}" ry="${DOT * 0.3}"/>`)
  }
  parts.push('</svg>')
  return parts.join('')
}

/** Every dark module outside the finder patterns, so the grid can be read. */
const darkModules: Array<[number, number]> = []
for (let row = 0; row < COUNT; row++) {
  for (let col = 7; col < COUNT; col++) {
    if (row < 7) continue
    darkModules.push([row, col])
  }
}

describe('readGrid', () => {
  it('reads the cell width and the module count back from the markup', () => {
    const grid = readGrid(svgWith(darkModules), SIZE)

    expect(grid).not.toBeNull()
    expect(grid!.count).toBe(COUNT)
    expect(grid!.cell).toBeCloseTo(CELL, 6)
    expect(grid!.margin).toBe(QR_MARGIN)
  })

  it('refuses markup with no modules in it', () => {
    expect(readGrid('<svg width="280" height="280"></svg>', SIZE)).toBeNull()
  })
})

describe('coverBox', () => {
  const grid = readGrid(svgWith(darkModules), SIZE)!
  const plate = coverBox(grid, SIZE)!

  it('is a whole number of modules across', () => {
    expect(plate.side / CELL).toBeCloseTo(9, 6)
  })

  it('starts and ends between modules, so its edges are never inside a dot', () => {
    expect((plate.x - QR_MARGIN) % CELL).toBeCloseTo(0, 6)
    expect((SIZE - QR_MARGIN - (plate.x + plate.side)) % CELL).toBeCloseTo(0, 6)
  })

  it('is centred on the code', () => {
    expect(plate.x).toBeCloseTo((SIZE - plate.side) / 2, 6)
    expect(plate.share).toBeCloseTo(plate.side / SIZE, 6)
  })

  it('leaves every dot either wholly inside it or wholly outside it', () => {
    const left = plate.x
    const right = plate.x + plate.side
    let inside = 0
    let outside = 0

    for (const [row, col] of darkModules) {
      const x = QR_MARGIN + col * CELL + (CELL - DOT) / 2
      const y = QR_MARGIN + row * CELL + (CELL - DOT) / 2
      const whollyInside = x >= left - 1e-9 && x + DOT <= right + 1e-9
        && y >= plate.y - 1e-9 && y + DOT <= plate.y + plate.side + 1e-9
      const whollyOutside = x + DOT <= left + 1e-9 || x >= right - 1e-9
        || y + DOT <= plate.y + 1e-9 || y >= plate.y + plate.side - 1e-9

      expect(whollyInside || whollyOutside).toBe(true)
      if (whollyInside) inside++
      else outside++
    }

    // Nine modules across at 25 per side: 81 dots hidden, most of them visible.
    expect(inside).toBeGreaterThan(0)
    expect(outside).toBeGreaterThan(inside)
  })

  it('never grows past the code or past the finder patterns', () => {
    expect(coverBox(grid, SIZE, 40)!.side / CELL).toBeCloseTo(COUNT - 8, 6)
  })
})

describe('clearMiddle', () => {
  it('draws the plate over the modules inside it, on the code white', () => {
    const svg = svgWith(darkModules)
    const plate = coverBox(readGrid(svg, SIZE)!, SIZE)!
    const cleared = clearMiddle(svg, plate)

    expect(cleared.endsWith('</svg>')).toBe(true)
    expect(cleared).toContain(
      `<rect x="${plate.x}" y="${plate.y}" width="${plate.side}" height="${plate.side}" fill="#fff"/>`
    )
    // The plate is drawn last, so it covers the dots rather than sitting under them.
    expect(cleared.indexOf('fill="#fff"')).toBeGreaterThan(cleared.lastIndexOf('fill="black"'))
    // Reading the grid back is unaffected: the plate is a plain rect.
    expect(readGrid(cleared, SIZE)!.count).toBe(COUNT)
  })
})
