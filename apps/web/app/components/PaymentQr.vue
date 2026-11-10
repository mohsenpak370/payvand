<script setup lang="ts">
// `?inline` turns the logo into a data URI at build time, so the generated
// markup is a self-contained SVG that still shows the logo when exported or
// opened on its own. A plain `/logo.png` href would break outside the app.
import logoDataUrl from '~/assets/logo.png?inline'

const props = defineProps<{
  payload: string
  size?: number
}>()

const markup = ref<string | null>(null)

const qrSize = computed(() => props.size ?? 320)

// The code a phone camera understands: an ordinary web link to the checkout,
// so the camera app, a scanner app and a wallet's own scanner all land on the
// same page. Encoding `solana:` here would only work in a wallet.
const origin = useRequestURL().origin
const qrText = computed(() => {
  if (!props.payload) return ''
  return `${origin}${paymentLinkPath(props.payload)}`
})

/*
 * Square modules with the encoder's own four-module quiet zone — deliberately
 * not the rounded "styled" code this used to draw.
 *
 * A styled code (every module a dot covering 80% of its cell, rounded corners)
 * is still read by phone camera apps, which use tolerant ML decoders, but not
 * by the strict grid readers: the 20% gap between neighbours makes each module
 * a separate round blob, and once a camera softens the edges jsQR — the decoder
 * on /scan — can no longer line the grid up. An invoice's link is long (the
 * whole code rides in its path), so the modules are small and the gap is all
 * it takes. Measured against jsQR on the same code and size, plain modules read
 * where dots do not.
 */
watch(
  qrText,
  async (text) => {
    if (!text) return
    const { default: QRCode } = await import('qrcode')
    markup.value = await QRCode.toString(text, {
      type: 'svg',
      margin: 4,
      width: qrSize.value,
      errorCorrectionLevel: 'Q',
      color: { dark: '#000000', light: '#ffffff' }
    })
  },
  { immediate: true }
)

/*
 * The logo plate sits in the code's middle. Error correction level Q recovers
 * ~25% of the code, and the plate covers ~6% of its area, so the modules
 * underneath are recoverable — this is the one decoration that survives.
 *
 * The encoder draws in module units inside a viewBox, so the plate is sized
 * against that extent rather than the rendered pixel size.
 */
const qrWithLogo = computed(() => {
  const svg = markup.value
  if (!svg) return null

  const viewBox = /viewBox="0 0 ([\d.]+) [\d.]+"/.exec(svg)
  if (!viewBox) return svg
  const extent = Number(viewBox[1])

  const logoSize = extent * 0.2
  const padding = extent * 0.025
  const plateSize = logoSize + padding * 2
  const plateOffset = (extent - plateSize) / 2
  const logoOffset = (extent - logoSize) / 2
  const radius = Math.max(0.5, extent * 0.02)

  const logo = `<rect x="${plateOffset}" y="${plateOffset}" width="${plateSize}" height="${plateSize}" fill="#fff" rx="${radius}" ry="${radius}"/>`
    + `<image x="${logoOffset}" y="${logoOffset}" width="${logoSize}" height="${logoSize}" href="${logoDataUrl}" preserveAspectRatio="xMidYMid meet"/>`

  return svg.replace('</svg>', `${logo}</svg>`)
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
    Generating QR code…
  </p>
</template>
