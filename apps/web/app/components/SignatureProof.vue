<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'
import { explorerTxUrl } from '~/utils/explorer'

/**
 * The signature, as proof rather than decoration. A truncated line the buyer can
 * only read is not proof of anything: this one is short enough to sit on the
 * sheet, copies the whole signature, and opens it on the cluster's explorer.
 *
 * Paper has nothing to click, so the buttons drop out of the print view and the
 * full signature prints in place of the short one.
 */
const props = defineProps<{
  signature: string
  /** The cluster the payment landed on — the explorer link needs it. */
  cluster: string
  label: string
  copyLabel: string
  copiedLabel: string
  explorerLabel: string
}>()

/** Long enough to recognise and compare, short enough never to wrap the sheet. */
const short = computed(() =>
  props.signature.length > 20
    ? `${props.signature.slice(0, 8)}…${props.signature.slice(-8)}`
    : props.signature
)

const href = computed(() => explorerTxUrl(props.signature, props.cluster))

const copied = ref(false)
let reset: ReturnType<typeof setTimeout> | undefined

async function copy() {
  try {
    await navigator.clipboard.writeText(props.signature)
  } catch {
    // A refused clipboard leaves the signature selectable and the link there.
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
      :title="signature"
    >{{ short }}</span>
    <span class="hidden break-all font-mono text-default print:inline">{{ signature }}</span>
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
      <UButton
        v-if="href"
        size="xs"
        color="neutral"
        variant="ghost"
        icon="i-lucide-external-link"
        :to="href"
        target="_blank"
        rel="noopener noreferrer"
        :aria-label="explorerLabel"
        :title="explorerLabel"
      />
    </div>
  </div>
</template>
