/**
 * The hero's code carries the Payvand mark in its middle.
 *
 * The code is drawn as one rounded dot per module, and a mark simply laid on top
 * leaves the dots at its edge half covered — a shape a camera reads as neither a
 * module nor a gap. So the plate the mark sits on is cut on the code's own module
 * grid: it starts and ends between modules, which makes every dot either wholly
 * inside it (hidden) or wholly outside it (visible).
 */

/** The quiet zone the generator draws, in the SVG's own units. */
export const QR_MARGIN = 2

/** `dotSize = cellSize * 0.8` in the generator (`@vue-solana/core`). */
const DOT_SHARE = 0.8

export interface QrGrid {
  /** One module's width, in the SVG's own units. */
  cell: number
  /** Modules per side. */
  count: number
  /** The quiet zone, in the same units. */
  margin: number
}

/**
 * The grid behind the markup `useSolanaQrCode().generate` returned.
 *
 * The module dots are the only rects that repeat: the three finder patterns
 * contribute six rects in all, so the commonest width is the dot, and the cell
 * it sits in is that divided by the generator's 80% share.
 */
export function readGrid(svg: string, size: number): QrGrid | null {
  const widths = [
    ...svg.matchAll(/<rect [^>]*width="([\d.]+)"[^>]*rx="[\d.]+" ry="[\d.]+"/g)
  ].map(match => Number(match[1]))
  if (!widths.length) return null

  const seen = new Map<number, number>()
  for (const width of widths) seen.set(width, (seen.get(width) ?? 0) + 1)
  const commonest = [...seen.entries()].sort((a, b) => b[1] - a[1])[0]
  if (!commonest) return null

  const cell = commonest[0] / DOT_SHARE
  const count = Math.round((size - QR_MARGIN * 2) / cell)
  if (!Number.isFinite(cell) || count < 21) return null
  return { cell, count, margin: QR_MARGIN }
}

export interface QrPlate {
  /** Left and top edge, in the SVG's own units. */
  x: number
  y: number
  /** Width and height of the plate, a whole number of modules. */
  side: number
  /** The plate's width against the code, for sizing the mark over it. */
  share: number
}

/**
 * The mark's plate: `cells` modules across, centred.
 *
 * Both the code's side count and the plate's are odd, so the square is centred
 * on a module rather than on a seam and its edges land exactly between modules.
 * Nothing here can move it off the grid, which is the property the dots need.
 */
export function coverBox(grid: QrGrid, size: number, cells = 11): QrPlate | null {
  // Eight modules at least are left around it, so the finder patterns stay whole.
  let across = Math.min(cells, grid.count - 8)
  if (across % 2 === 0) across -= 1
  if (across < 3) return null
  if ((grid.count - across) % 2 !== 0) return null

  const side = across * grid.cell
  const x = (size - side) / 2
  return { x, y: x, side, share: side / size }
}

/**
 * The code with its middle cleared: a white plate on the code's own background,
 * drawn over the dots inside it and touching nothing outside it.
 */
export function clearMiddle(svg: string, plate: QrPlate): string {
  const rect = `<rect x="${plate.x}" y="${plate.y}" width="${plate.side}" height="${plate.side}" fill="#fff"/>`
  return svg.replace('</svg>', `${rect}</svg>`)
}
