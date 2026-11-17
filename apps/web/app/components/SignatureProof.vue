<script setup lang="ts">
import { computed } from 'vue'
import { explorerTxUrl } from '~/utils/explorer'

/**
 * The signature, as proof rather than decoration. A truncated line the buyer can
 * only read is not proof of anything: this one is short enough to sit on the
 * sheet, copies the whole signature, and opens it on the cluster's explorer.
 *
 * The line itself is `CopyField`, the same row the reference code and the payer's
 * wallet use, so every value the sheet hands over copies the same way — and paper
 * keeps the full one where the buttons drop out.
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

const href = computed(() => explorerTxUrl(props.signature, props.cluster))
</script>

<template>
  <CopyField
    :label="label"
    :value="signature"
    :keep="8"
    :copy-label="copyLabel"
    :copied-label="copiedLabel"
  >
    <template #actions>
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
    </template>
  </CopyField>
</template>
