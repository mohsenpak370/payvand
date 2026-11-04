<script setup lang="ts">
// `?inline` turns the logo into a data URI at build time, so the generated
// markup is a self-contained SVG that still shows the logo when exported or
// opened on its own. A plain `/logo.png` href would break outside the app.
import logoDataUrl from '~/assets/logo.png?inline'

const props = defineProps<{
  payload: string
  size?: number
}>()

const { generate, loading } = useSolanaQrCode()
const markup = ref<string | null>(null)

const qrSize = computed(() => props.size ?? 320)

// The encoder uses error-correction level "Q" (~25% recoverable), so a logo
// covering ~4% of the area leaves plenty of margin for reliable scans.
const qrWithLogo = computed(() => {
  if (!markup.value) return null

  const size = qrSize.value
  const logoSize = Math.round(size * 0.2)
  const padding = Math.round(size * 0.025)
  const plateSize = logoSize + padding * 2
  const plateOffset = (size - plateSize) / 2
  const logoOffset = (size - logoSize) / 2
  const radius = Math.max(2, Math.round(size * 0.02))

  const logo = `<rect x="${plateOffset}" y="${plateOffset}" width="${plateSize}" height="${plateSize}" fill="#fff" rx="${radius}" ry="${radius}"/>`
    + `<image x="${logoOffset}" y="${logoOffset}" width="${logoSize}" height="${logoSize}" href="${logoDataUrl}" preserveAspectRatio="xMidYMid meet"/>`

  return markup.value.replace('</svg>', `${logo}</svg>`)
})

watch(
  () => props.payload,
  async (payload) => {
    if (!payload) return
    markup.value = await generate(payload, { size: qrSize.value })
  },
  { immediate: true }
)
</script>

<template>
  <div
    v-if="qrWithLogo"
    class="rounded-lg bg-white p-3"
    v-html="qrWithLogo"
  />
  <p
    v-else
    class="text-sm text-muted"
  >
    {{ loading ? 'Generating QR code…' : 'Generating QR code…' }}
  </p>
</template>
