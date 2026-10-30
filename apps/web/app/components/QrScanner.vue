<script setup lang="ts">
import jsQR from 'jsqr'

const emit = defineEmits<{
  decoded: [text: string]
}>()

const videoRef = ref<HTMLVideoElement>()
const message = ref('')
const scanning = ref(false)

let stream: MediaStream | null = null
let frame = 0
let canvas: HTMLCanvasElement | null = null
let context: CanvasRenderingContext2D | null = null

async function start() {
  if (scanning.value) return
  message.value = ''

  // Browsers only expose the camera in a secure context: over plain
  // http:// on a LAN address `mediaDevices` doesn't exist at all, and the
  // permission prompt will never appear. Say so instead of pretending the
  // user denied something.
  if (!navigator.mediaDevices?.getUserMedia) {
    message.value = 'Camera needs a secure connection — open this page over https (or localhost).'
    return
  }

  try {
    stream = await navigator.mediaDevices.getUserMedia({
      video: { facingMode: 'environment' },
      audio: false
    })
  } catch {
    message.value = 'Camera permission was denied or no camera is available.'
    return
  }

  const video = videoRef.value
  if (!video) return

  canvas = document.createElement('canvas')
  context = canvas.getContext('2d', { willReadFrequently: true })
  video.srcObject = stream
  await video.play()
  scanning.value = true
  tick()
}

function tick() {
  if (!scanning.value) return

  const video = videoRef.value
  if (video && context && canvas && video.readyState === video.HAVE_ENOUGH_DATA) {
    canvas.width = video.videoWidth
    canvas.height = video.videoHeight
    context.drawImage(video, 0, 0, canvas.width, canvas.height)

    const image = context.getImageData(0, 0, canvas.width, canvas.height)
    const code = jsQR(image.data, image.width, image.height, {
      inversionAttempts: 'attemptBoth'
    })

    if (code?.data) {
      stop()
      emit('decoded', code.data)
      return
    }
  }

  frame = requestAnimationFrame(tick)
}

function stop() {
  scanning.value = false
  cancelAnimationFrame(frame)
  for (const track of stream?.getTracks() ?? []) track.stop()
  stream = null
  if (videoRef.value) videoRef.value.srcObject = null
}

onMounted(start)
onBeforeUnmount(stop)

defineExpose({ start, stop })
</script>

<template>
  <div class="overflow-hidden rounded-xl border border-default">
    <video
      ref="videoRef"
      playsinline
      muted
      class="aspect-square w-full bg-black object-cover"
    />
    <div
      class="flex items-center justify-between gap-2 border-t border-default px-3 py-2 text-sm"
    >
      <span class="text-muted">
        {{ message || (scanning ? 'Point the camera at the payment code.' : 'Camera idle.') }}
      </span>
      <UButton
        v-if="message || !scanning"
        size="xs"
        color="neutral"
        variant="soft"
        label="Retry"
        @click="start"
      />
    </div>
  </div>
</template>
