<script setup lang="ts">
/**
 * What a payment looks like once it lands, drawn for the hero.
 *
 * The code is a real one: it points at this site's home page, so anyone who
 * scans it from the hero lands in the app rather than on a dead square. It is
 * generated in the browser because the address depends on where the page is
 * served from, and the pattern underneath keeps the card's shape until the
 * code is drawn.
 */
const { generate } = useSolanaQrCode()

/** The code is drawn in its own pixels, and the mark is placed in the same ones. */
const QR_SIZE = 280
/**
 * How wide the mark's art is against the code. The code is drawn at error
 * correction level Q, which recovers ~25% of it; a mark this size covers ~4%
 * of the area, so the modules behind it come back and the code still scans.
 */
const MARK_SHARE = 0.2
/** The art sits inset in the logo's own square: about 70% of it is the tile. */
const MARK_ART_SHARE = 0.7

const qrMarkup = ref<string | null>(null)

const qrCells = [
  0, 1, 1, 0, 1, 0, 1,
  1, 0, 1, 1, 0, 1, 0,
  1, 1, 0, 1, 1, 0, 1,
  0, 1, 1, 0, 1, 1, 0,
  1, 0, 1, 1, 0, 1, 1,
  1, 1, 0, 1, 0, 0, 1,
  0, 1, 1, 0, 1, 1, 0
]

onMounted(async () => {
  const origin = window.location.origin
  try {
    qrMarkup.value = await generate(`${origin}/`, { size: QR_SIZE })
  } catch {
    // A card that cannot draw the code keeps the pattern below instead.
    qrMarkup.value = null
  }
})

/**
 * The mark belongs to the code, not on top of it: it goes into the code's own
 * SVG, in the middle of the modules, with nothing drawn around it. A white
 * plate and a margin of its own are what make a logo look pasted on.
 */
const qrWithMark = computed(() => {
  const svg = qrMarkup.value
  if (!svg) return null
  const markSize = Math.round(QR_SIZE * MARK_SHARE / MARK_ART_SHARE)
  const offset = (QR_SIZE - markSize) / 2
  return svg.replace(
    '</svg>',
    `${markMarkup({ x: offset, y: offset, size: markSize })}</svg>`
  )
})
</script>

<template>
  <UCard class="shadow-lg shadow-primary/10">
    <div class="flex flex-col gap-4">
      <div class="flex items-center justify-between gap-3">
        <span class="text-muted text-xs">{{ $t('home.hero.card.example') }}</span>
        <UBadge
          color="success"
          variant="subtle"
          size="sm"
          icon="i-lucide-circle-check"
        >
          {{ $t('home.hero.card.paid') }}
        </UBadge>
      </div>

      <div class="flex items-start justify-between gap-4">
        <div class="min-w-0">
          <p class="truncate text-sm font-medium">
            {{ $t('home.hero.card.memo') }}
          </p>
          <p class="text-muted mt-2 text-xs">
            {{ $t('home.hero.card.code') }}
            <span class="text-default ms-1 font-mono tracking-widest">PV7K2XQ4</span>
          </p>
        </div>
        <div class="shrink-0 text-end">
          <p class="text-2xl font-semibold tabular-nums">
            4.50
          </p>
          <p class="text-muted text-xs">
            USDC
          </p>
        </div>
      </div>

      <!--
        A white plate, because a code stays readable only against it. The
        margin is the code's quiet zone, not decoration: the modules need to
        end against plain white for a camera to find the edges.
      -->
      <div
        v-if="qrWithMark"
        class="mx-auto w-fit max-w-full rounded-xl bg-white p-3 [&_svg]:h-auto [&_svg]:max-h-[280px] [&_svg]:w-auto [&_svg]:max-w-full"
        v-html="qrWithMark"
      />
      <div
        v-else
        class="border-default grid grid-cols-7 gap-1 rounded-xl border p-2"
        aria-hidden="true"
      >
        <div
          v-for="(filled, index) in qrCells"
          :key="index"
          class="aspect-square rounded-[2px]"
          :class="filled ? 'bg-default' : 'bg-transparent'"
        />
      </div>
    </div>
  </UCard>
</template>
