<script setup lang="ts">
const props = defineProps<{
  payload: string
  size?: number
}>()

const { svg, generate, loading } = useSolanaQrCode()

watch(
  () => props.payload,
  async (payload) => {
    if (!payload) return
    await generate(payload, { size: props.size ?? 320 })
  },
  { immediate: true }
)
</script>

<template>
  <div
    v-if="svg"
    class="rounded-lg bg-white p-3"
    v-html="svg"
  />
  <p
    v-else
    class="text-sm text-muted"
  >
    {{ loading ? 'Generating QR code…' : 'Generating QR code…' }}
  </p>
</template>
