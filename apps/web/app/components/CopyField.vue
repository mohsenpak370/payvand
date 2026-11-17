<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'

/**
 * A value the sheet hands over rather than only shows: shortened to fit the
 * card, printed in full, and copied whole. The signature, the reference code and
 * the wallet that paid all read this way, so a buyer copies any of them the same.
 *
 * Paper has nothing to click, so the button drops out of the print view and the
 * whole value prints in place of the shortened one.
 */
const props = withDefaults(
  defineProps<{
    label: string
    value: string
    copyLabel: string
    copiedLabel: string
    /*
     * Characters kept at each end on the screen. 0 shows the value whole: an
     * eight-character reference code has nothing worth hiding.
     */
    keep?: number
  }>(),
  { keep: 0 }
)

/** Long enough to recognise and compare, short enough never to wrap the sheet. */
const shown = computed(() => {
  const keep = props.keep
  if (!keep || props.value.length <= keep * 2 + 1) return props.value
  return `${props.value.slice(0, keep)}…${props.value.slice(-keep)}`
})

const copied = ref(false)
let reset: ReturnType<typeof setTimeout> | undefined

async function copy() {
  try {
    await navigator.clipboard.writeText(props.value)
  } catch {
    // A refused clipboard leaves the value selectable in place.
    return
  }
  copied.value = true
  clearTimeout(reset)
  reset = setTimeout(() => {
    copied.value = false
  }, 2_000)
}

onBeforeUnmount(() => clearTimeout(reset))
</script>

<template>
  <div class="flex items-center gap-2 text-xs text-muted">
    <span class="shrink-0">{{ label }}</span>
    <span
      class="font-mono text-default print:hidden"
      :title="value"
    >{{ shown }}</span>
    <span class="hidden break-all font-mono text-default print:inline">{{ value }}</span>
    <div class="ml-auto flex shrink-0 items-center gap-1 print:hidden">
      <UButton
        size="xs"
        color="neutral"
        variant="ghost"
        :icon="copied ? 'i-lucide-check' : 'i-lucide-copy'"
        :aria-label="copied ? copiedLabel : copyLabel"
        :title="copied ? copiedLabel : copyLabel"
        @click="copy"
      />
      <slot name="actions" />
    </div>
  </div>
</template>
