/**
 * The Payvand mark as SVG markup, for the places that carry it *inside*
 * another SVG: the hero's code has it in the middle of its own modules, so the
 * logo is part of the code rather than a picture laid over one.
 *
 * The light that runs the ribbon is CSS, and injected markup cannot borrow a
 * component's scoped style — so the rules travel with the markup, under names
 * of their own. That also keeps the code readable when the SVG is saved or
 * printed on its own.
 */
const VIEWBOX = 1254

/** The navy tile the ribbons sit on, in the logo's own 1254-unit grid. */
const TILE = 'M981.0 1023.5 L971.0 1031.2 L954.0 1042.2 L939.0 1050.3 L923.0 1057.2 L900.0 1064.5 L886.0 1067.5 L864.0 1070.5 L852.0 1070.5 L847.0 1071.5 L405.0 1071.5 L401.0 1070.5 L390.0 1070.5 L387.0 1069.5 L373.0 1068.5 L358.0 1065.5 L346.0 1062.2 L321.0 1053.2 L298.0 1041.2 L282.7 1031.0 L267.0 1018.5 L250.5 1002.0 L240.0 989.3 L229.8 975.0 L220.8 960.0 L211.8 942.0 L206.8 930.0 L200.8 912.0 L195.5 891.0 L192.5 872.0 L192.5 864.0 L191.5 860.0 L191.5 842.0 L190.5 838.0 L190.5 414.0 L191.5 409.0 L191.5 385.0 L192.5 382.0 L192.5 374.0 L194.5 360.0 L198.8 340.0 L208.8 310.0 L219.8 287.0 L234.8 263.0 L251.5 242.0 L267.0 226.5 L283.7 213.0 L296.0 204.8 L308.0 197.8 L325.0 189.8 L341.0 183.8 L359.0 178.8 L376.0 175.5 L389.0 174.5 L392.0 173.5 L403.0 173.5 L408.0 172.5 L846.0 172.5 L850.0 173.5 L862.0 173.5 L865.0 174.5 L871.0 174.5 L889.0 177.5 L906.0 181.8 L921.0 186.8 L933.0 191.8 L954.0 202.8 L969.3 213.0 L984.0 224.5 L1005.5 246.0 L1017.0 260.7 L1026.2 274.0 L1032.2 284.0 L1042.2 304.0 L1048.2 319.0 L1053.2 335.0 L1057.5 352.0 L1060.5 372.0 L1060.5 380.0 L1061.5 384.0 L1061.5 399.0 L1062.5 403.0 L1062.5 842.0 L1061.5 846.0 L1061.5 861.0 L1060.5 865.0 L1060.5 872.0 L1057.5 892.0 L1052.2 913.0 L1041.2 943.0 L1029.2 966.0 L1018.0 983.3 L997.5 1008.0Z'

const VR = 'M790.0 790.7 L788.6 788.0 L787.2 776.0 L784.3 768.0 L743.0 699.1 L742.9 695.0 L863.0 486.5 L872.0 478.8 L878.0 476.6 L943.0 476.5 L949.0 478.3 L955.4 484.0 L958.2 489.0 L959.5 495.0 L957.3 503.0 L794.2 788.0 L792.0 790.7Z'

const VL = 'M757.0 824.2 L746.0 826.4 L739.0 826.5 L725.0 823.2 L718.0 820.2 L711.7 816.0 L699.6 804.0 L612.9 660.0 L612.9 658.0 L615.0 656.8 L629.0 655.4 L643.0 651.2 L658.0 643.2 L675.4 627.0 L686.0 610.8 L688.4 612.0 L697.2 623.0 L741.0 694.2 L744.0 695.7 L783.2 760.0 L788.2 771.0 L792.0 791.0 L787.2 800.0 L777.0 812.4 L768.0 819.2Z'

const P = 'M339.0 821.2 L334.0 814.5 L331.5 807.0 L331.5 451.0 L333.8 439.0 L339.6 427.0 L353.0 414.8 L359.0 411.8 L371.0 408.5 L572.0 408.5 L591.0 412.8 L615.0 423.8 L635.3 437.0 L653.0 452.5 L669.2 473.0 L683.2 499.0 L692.4 528.0 L695.5 547.0 L695.5 573.0 L692.2 593.0 L687.0 609.0 L687.1 613.0 L678.0 628.3 L665.0 641.4 L657.0 647.2 L645.0 653.2 L636.0 656.2 L615.0 659.4 L609.0 657.5 L570.0 657.5 L552.0 661.5 L550.8 660.0 L551.8 658.0 L575.3 642.0 L587.4 631.0 L596.7 618.0 L604.2 600.0 L606.5 587.0 L606.5 566.0 L605.0 561.0 L605.5 556.0 L604.4 549.0 L599.7 535.0 L590.4 519.0 L580.0 508.6 L573.0 503.8 L558.0 497.6 L542.0 496.5 L535.0 497.5 L522.0 496.5 L514.0 497.5 L506.0 496.6 L498.0 497.5 L489.0 496.7 L450.0 497.5 L436.0 496.5 L423.5 498.0 L422.5 501.0 L422.5 654.0 L421.5 661.0 L421.5 774.0 L422.5 780.0 L422.5 804.0 L421.2 811.0 L416.2 820.0 L410.5 824.0 L404.0 826.4 L351.0 826.5 L345.6 825.0Z'

const RB = 'M421.0 781.2 L419.5 776.0 L420.0 656.5 L425.0 655.4 L435.5 646.0 L487.0 595.6 L502.7 583.0 L513.0 576.7 L531.0 567.8 L543.0 563.8 L558.0 560.6 L567.0 559.5 L607.0 560.4 L608.5 564.0 L608.5 590.0 L606.2 602.0 L598.7 620.0 L589.4 633.0 L577.0 644.2 L551.0 662.2 L538.0 669.6 L425.5 779.0Z'

/** The route the light takes: out from under the P's stem, under its bowl,
 * down the V's left arm and up its right arm to the tip. */
const RIBBON = 'M396.0 745.5 C401.0 740.7 419.3 722.9 426.0 716.5 C432.7 710.1 432.7 710.2 436.0 707.0 C439.3 703.8 442.7 700.2 446.0 697.0 C449.3 693.8 452.7 690.7 456.0 687.5 C459.3 684.3 462.7 681.2 466.0 678.0 C469.3 674.8 472.7 671.2 476.0 668.0 C479.3 664.8 482.7 661.6 486.0 658.5 C489.3 655.4 492.7 652.4 496.0 649.5 C499.3 646.6 502.7 643.8 506.0 641.0 C509.3 638.2 512.7 635.5 516.0 633.0 C519.3 630.5 522.7 628.3 526.0 626.0 C529.3 623.7 532.7 621.0 536.0 619.0 C539.3 617.0 542.7 615.8 546.0 614.0 C549.3 612.2 552.7 609.7 556.0 608.0 C559.3 606.3 562.7 605.2 566.0 604.0 C569.3 602.8 572.7 601.8 576.0 600.5 C579.3 599.2 584.3 596.8 586.0 596.0 L646.1 628.0 L728.1 760.5 Q746.0 789.5 762.6 759.8 L904.9 506.8'

const RAMP = [0, 0.167, 0.278, 0.389, 0.5, 0.611, 0.722, 0.833, 1]

interface Gradient {
  x1: number
  y1: number
  x2: number
  y2: number
  colors: string[]
}

/** Each part of the mark, in the order the logo paints them. */
const GRADIENTS: Record<string, Gradient> = {
  vr: {
    x1: 750.7,
    y1: 735.2,
    x2: 942.5,
    y2: 486.8,
    colors: ['#516bfd', '#4d72fd', '#497afd', '#4482fd', '#3f8bfd', '#3b93fd', '#399bfd', '#38a1fd', '#37a8fd']
  },
  vl: {
    x1: 766,
    y1: 813.7,
    x2: 650.9,
    y2: 643.4,
    colors: ['#3ebffd', '#3ebefd', '#3bbafd', '#39b2fd', '#36a9fd', '#339cfd', '#308efd', '#307bfd', '#2d5ee9']
  },
  p: {
    x1: 344,
    y1: 444.7,
    x2: 605.7,
    y2: 684.9,
    colors: ['#3dbcfd', '#3bb4fd', '#37a8fd', '#3698fd', '#3e88fd', '#4a7afd', '#526dfc', '#3d5ef8', '#314cf2']
  },
  rb: {
    x1: 612.2,
    y1: 617.7,
    x2: 403.4,
    y2: 678,
    colors: ['#3ec1fd', '#3dbefd', '#39b3fd', '#35a4fd', '#3393fd', '#3880fd', '#416bfc', '#4157f7', '#3149e2']
  }
}

/**
 * One run of the light every 4.6s: ~2.8s along the ribbon, then the mark rests.
 * The dash offset moves the light, the group fade hides it while parked.
 */
const ANIMATION = `
@keyframes pvMarkRun{0%{stroke-dashoffset:0}60%,100%{stroke-dashoffset:-1000}}
@keyframes pvMarkFade{0%,4%{opacity:0}10%,54%{opacity:1}61%,100%{opacity:0}}
.pvMarkLight{animation:pvMarkFade 4.6s linear infinite}
.pvMarkDash{animation:pvMarkRun 4.6s cubic-bezier(.45,0,.35,1) infinite}
@media (prefers-reduced-motion:reduce){.pvMarkLight{display:none}}
`

export interface MarkPlacement {
  /** Left and top edge, in the target SVG's own units. */
  x: number
  y: number
  /** The mark's width and height, in the same units. */
  size: number
  /** Prefix for the gradient ids, so two marks on one page cannot collide. */
  id?: string
}

/**
 * The mark, ready to drop inside an `<svg>`: gradients, the animation's rules,
 * and the mark itself scaled into place. Nothing is drawn around it — no plate,
 * no padding — so what sits in the code's middle is the logo's own edge.
 */
export function markMarkup({ x, y, size, id = 'pvMark' }: MarkPlacement): string {
  const scale = size / VIEWBOX

  const gradients = Object.entries(GRADIENTS)
    .map(([key, gradient]) => {
      const stops = gradient.colors
        .map((color, index) => `<stop offset="${RAMP[index]}" stop-color="${color}"/>`)
        .join('')
      return `<linearGradient id="${id}${key}" gradientUnits="userSpaceOnUse" x1="${gradient.x1}" y1="${gradient.y1}" x2="${gradient.x2}" y2="${gradient.y2}">${stops}</linearGradient>`
    })
    .join('')

  // Both light strokes follow the same measured route: `pathLength` makes the
  // dash a share of the ribbon rather than a number in user units.
  const light = 'path-length="1000" fill="none" stroke-linecap="round" stroke-dasharray="70 930" class="pvMarkDash"'

  return [
    `<defs>${gradients}</defs>`,
    `<style>${ANIMATION}</style>`,
    `<g transform="translate(${x} ${y}) scale(${scale})" aria-hidden="true">`,
    `<path d="${TILE}" fill="#080e20"/>`,
    `<path d="${VR}" fill="url(#${id}vr)"/>`,
    `<path d="${VL}" fill="url(#${id}vl)"/>`,
    `<path d="${P}" fill="url(#${id}p)"/>`,
    `<path d="${RB}" fill="url(#${id}rb)"/>`,
    '<g class="pvMarkLight">',
    `<path d="${RIBBON}" ${light} stroke="#4bbdfd" stroke-width="62" opacity="0.4"/>`,
    `<path d="${RIBBON}" ${light} stroke="#d8f4ff" stroke-width="26"/>`,
    '</g>',
    '</g>'
  ].join('')
}
