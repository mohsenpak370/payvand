<script setup lang="ts">
// `?inline` turns the logo into a data URI at build time, so the generated
// markup is a self-contained SVG that still shows the logo when exported or
// opened on its own. A plain `/logo.png` href would break outside the app.
import logoDataUrl from '~/assets/logo.png?inline'
import { clearMiddle, coverBox, readGrid } from '~/utils/qrPlate'

const props = defineProps<{
  /** An invoice's payload, encoded as its checkout link. */
  payload?: string
  /**
   * A path to encode as it is, with the origin in front of it — a reusable
   * code, whose link is the code: opening it mints this visit's invoice.
   */
  url?: string
  size?: number
}>()

const { generate } = useSolanaQrCode()
const markup = ref<string | null>(null)

const qrSize = computed(() => props.size ?? 320)

// The code a phone camera understands: an ordinary web link to the checkout,
// so the camera app, a scanner app and a wallet's own scanner all land on the
// same page. Encoding `solana:` here would only work in a wallet.
const origin = useRequestURL().origin
const qrText = computed(() => {
  if (props.url) return `${origin}${props.url}`
  if (!props.payload) return ''
  return `${origin}${paymentLinkPath(props.payload)}`
})

/*
 * The styled code: every module a rounded dot covering 80% of its cell, with
 * extra-rounded finder patterns. It is the app's own look, and it survives
 * because /scan no longer leans on a JavaScript grid reader — rxing (ZXing in
 * WebAssembly) reads it through a lens at the size this sheet shows, where the
 * native detector alone does not: measured at 400px on a lens-softened 1080p
 * frame of a real invoice, both read the styled code, and at 320px only rxing
 * does. The size matters, so keep it at 400 rather than a thumbnail; a 240px
 * code is unreadable to every decoder once a lens softens it.
 */
watch(
  qrText,
  async (text) => {
    if (!text) return
    markup.value = await generate(text, { size: qrSize.value })
  },
  { immediate: true }
)

/*
 * The mark sits in the code's middle, on a clearing cut on the code's own
 * module grid — `coverBox` returns a square that starts and ends between
 * modules, so every dot is either wholly under the mark or wholly visible
 * beside it, never half covered.
 *
 * The mark is drawn at the clearing's exact width: its tile fills the square,
 * with no white frame left showing around it. Error correction level Q recovers
 * ~25% of the code, and the plate hides about a tenth of its area, so what is
 * under the tile is recoverable.
 */

/**
 * The plate's width as a share of the code's side, before snapping to whole
 * modules. A share of the side count (rather than a fixed module count) keeps
 * the mark the same visual size on small and large codes, and keeps the hidden
 * area well inside what error correction level Q can recover.
 */
const LOGO_SHARE = 0.28

const qrWithLogo = computed(() => {
  const svg = markup.value
  if (!svg) return null

  const size = qrSize.value
  const grid = readGrid(svg, size)
  // Odd, so the plate is centred on a module instead of on a seam between two.
  let cells = Math.ceil(grid ? grid.count * LOGO_SHARE : 0)
  if (cells % 2 === 0) cells += 1

  const plate = grid ? coverBox(grid, size, cells) : null
  if (!plate) return null

  const radius = plate.side * 0.05
  const logo = `<image x="${plate.x}" y="${plate.y}" width="${plate.side}" height="${plate.side}" rx="${radius}" ry="${radius}" href="${logoDataUrl}" preserveAspectRatio="xMidYMid meet"/>`

  return clearMiddle(svg, plate).replace('</svg>', `${logo}</svg>`)
})
</script>

<template>
  <!--
    The code is drawn at a fixed size for scanners, but still has to fit a
    narrow phone: the SVG shrinks with `max-w-full` rather than pushing the
    card sideways, and scales back up wherever there is room.
  -->
  <div
    v-if="qrWithLogo"
    class="mx-auto w-fit max-w-full rounded-lg bg-white p-3 [&_svg]:h-auto [&_svg]:max-w-full"
    v-html="qrWithLogo"
  />
  <p
    v-else
    class="text-sm text-muted"
  >
    {{ $t('qr.generating') }}
  </p>
</template>
