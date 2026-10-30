<script setup lang="ts">
import QRCode from 'qrcode'

const props = defineProps<{
  payload: string
  size?: number
}>()

// The QR is a link to the pay page: scanning it with any camera opens the
// invoice in a browser instead of showing a blob only this app understands.

const src = ref('')

watch(
  () => props.payload,
  async (payload) => {
    if (!payload) {
      src.value = ''
      return
    }
    try {
      src.value = await QRCode.toDataURL(paymentUrl(payload), {
        width: props.size ?? 320,
        margin: 2,
        errorCorrectionLevel: 'M',
        color: { dark: '#000000ff', light: '#ffffffff' }
      })
    } catch {
      src.value = ''
    }
  },
  { immediate: true }
)
</script>

<template>
  <img
    v-if="src"
    :src="src"
    :width="size ?? 320"
    :height="size ?? 320"
    alt="Payment QR code"
    class="rounded-lg bg-white p-3"
  >
  <p
    v-else
    class="text-sm text-muted"
  >
    Generating QR code…
  </p>
</template>
