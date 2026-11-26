<script setup lang="ts">
import { explorerAddressUrl, explorerTxUrl } from '~/utils/explorer'
import { shorten } from '~/utils/profileFormat'

/**
 * An address or signature in an admin table: shortened to fit, whole on hover,
 * copied whole, and one click from the explorer on the cluster it lives on.
 */
const props = withDefaults(
  defineProps<{
    value: string
    cluster: string
    kind?: 'address' | 'tx'
  }>(),
  { kind: 'address' }
)

const { t } = useI18n()

const href = computed(() =>
  props.kind === 'tx'
    ? explorerTxUrl(props.value, props.cluster)
    : explorerAddressUrl(props.value, props.cluster)
)

const copied = shallowRef(false)
let reset: ReturnType<typeof setTimeout> | undefined

async function copy() {
  try {
    await navigator.clipboard.writeText(props.value)
  } catch {
    // A refused clipboard leaves the value readable in the tooltip.
    return
  }
  copied.value = true
  clearTimeout(reset)
  reset = setTimeout(() => {
    copied.value = false
  }, 2_000)
}

onBeforeUnmount(() => clearTimeout(reset))

const copyLabel = computed(() => (copied.value ? t('admin.address.copied') : t('admin.address.copy')))
</script>

<template>
  <span class="inline-flex items-center gap-0.5 whitespace-nowrap">
    <span
      class="font-mono text-xs"
      :title="value"
    >{{ shorten(value) }}</span>
    <UButton
      size="xs"
      color="neutral"
      variant="ghost"
      :icon="copied ? 'i-lucide-check' : 'i-lucide-copy'"
      :aria-label="copyLabel"
      :title="copyLabel"
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
      :aria-label="$t('admin.address.explorer')"
      :title="$t('admin.address.explorer')"
    />
  </span>
</template>
