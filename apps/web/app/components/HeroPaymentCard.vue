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
 * The code with its middle cleared for the mark, and how wide the mark is
 * against it.
 *
 * The plate is a whole number of modules across and centred on the grid, so no
 * dot is left half covered by it: each one is either under the plate or clear
 * of it. The code is drawn at error correction level Q, which recovers ~25% of
 * it, and eleven modules across hides about 19% of the code's area.
 */
const qrPlate = computed(() => {
  const svg = qrMarkup.value
  if (!svg) return null
  const grid = readGrid(svg, QR_SIZE)
  const plate = grid ? coverBox(grid, QR_SIZE) : null
  if (!plate) return null
  return {
    markup: clearMiddle(svg, plate),
    share: plate.share,
    /** The mark is drawn at the plate's width, so its tile fills the clearing. */
    markSize: `${(plate.share * 100).toFixed(2)}%`
  }
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
        v-if="qrPlate"
        class="mx-auto w-fit max-w-full rounded-xl bg-white p-3"
      >
        <div class="relative aspect-square w-[280px] max-w-full">
          <div
            class="h-full w-full [&_svg]:h-full [&_svg]:w-full"
            v-html="qrPlate.markup"
          />
          <!-- The drawn mark sits in the clearing, centred on the same grid. -->
          <div class="pointer-events-none absolute inset-0 flex items-center justify-center">
            <PayvandLogoDraw
              :size="qrPlate.markSize"
              loop
            />
          </div>
        </div>
      </div>
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
