<script setup lang="ts">
// `?inline` turns the logo into a data URI at build time, so the generated
// markup is a self-contained SVG that still shows the logo when exported or
// opened on its own. A plain `/logo.png` href would break outside the app.
import logoDataUrl from '~/assets/logo.png?inline'

const props = defineProps<{
  payload: string
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
 * The logo plate sits in the code's middle. Error correction level Q recovers
 * ~25% of the code, and the plate covers ~6% of its area, so the modules
 * underneath are recoverable — this is the one decoration that survives.
 *
 * The encoder draws in pixels here (the styled SVG's viewBox is the size
 * itself), so the plate is scaled from the same number.
 */
const qrWithLogo = computed(() => {
  const svg = markup.value
  if (!svg) return null

  const size = qrSize.value
  const logoSize = Math.round(size * 0.2)
  const padding = Math.round(size * 0.025)
  const plateSize = logoSize + padding * 2
  const plateOffset = (size - plateSize) / 2
  const logoOffset = (size - logoSize) / 2
  const radius = Math.max(2, Math.round(size * 0.02))

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
    {{ $t('qr.generating') }}
  </p>
</template>
