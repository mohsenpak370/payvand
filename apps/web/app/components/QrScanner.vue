<script setup lang="ts">
import jsQR from 'jsqr'

const emit = defineEmits<{
  decoded: [text: string]
}>()

const videoRef = ref<HTMLVideoElement>()
const message = ref('')
const scanning = ref(false)

let stream: MediaStream | null = null
let waitNotice = 0
let frame = 0
let canvas: HTMLCanvasElement | null = null
let context: CanvasRenderingContext2D | null = null

async function start() {
  if (scanning.value) return
  message.value = ''
  clearTimeout(waitNotice)

  // Browsers only expose the camera in a secure context: over plain
  // http:// on a LAN address `mediaDevices` doesn't exist at all, and the
  // permission prompt will never appear. Say so instead of pretending the
  // user denied something.
  if (!navigator.mediaDevices?.getUserMedia) {
    message.value = 'Camera needs a secure connection — open this page over https (or localhost).'
    return
  }

  /*
   * A prompt nobody answers leaves the request pending forever, and the panel
   * would sit on "Starting the camera…" for as long as the tab is open. Say
   * what to do about it instead.
   */
  waitNotice = window.setTimeout(() => {
    if (!scanning.value) {
      message.value = 'The camera has not started. Allow camera access for this site, then try again.'
    }
  }, 15_000)

  try {
    // `ideal` rather than a hard requirement: on a laptop there is no back
    // camera, and asking for one as a requirement fails the whole request.
    stream = await navigator.mediaDevices.getUserMedia({
      video: { facingMode: { ideal: 'environment' } },
      audio: false
    })
  } catch {
    clearTimeout(waitNotice)
    message.value = 'Camera permission was denied or no camera is available.'
    return
  }
  clearTimeout(waitNotice)

  /*
   * Phones often keep a fixed focus by default, so a code has to be held
   * further away than the frame asks for to stay sharp. Ask for continuous
   * autofocus; browsers without it ignore the request, and one that refuses is
   * no reason to stop scanning.
   */
  const [track] = stream.getVideoTracks()
  try {
    await track?.applyConstraints({
      advanced: [{ focusMode: 'continuous' } as unknown as MediaTrackConstraintSet]
    })
  } catch {
    // Keep the camera's own choice of focus.
  }

  const video = videoRef.value
  if (!video) return

  canvas = document.createElement('canvas')
  context = canvas.getContext('2d', { willReadFrequently: true })
  video.srcObject = stream
  /*
   * `play()` can stay pending until the page gets a gesture (iOS is the usual
   * offender), and awaiting it meant the panel sat on "Starting the camera…"
   * with a working camera behind it. Let it resolve on its own: `tick()` waits
   * for frames anyway, and replays are silent if the browser refuses.
   */
  void video.play().catch(() => {})
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
  clearTimeout(waitNotice)
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
  <!--
    A viewfinder rather than a video box: the camera fills a dark panel and a
    square in the middle is the only part left undimmed, which is how phone
    scanner apps show where the code belongs. Taller than it is wide on a
    phone, square once there is room.
  -->
  <div class="overflow-hidden rounded-2xl bg-neutral-950">
    <div class="relative aspect-[4/5] w-full sm:aspect-square">
      <video
        ref="videoRef"
        autoplay
        playsinline
        webkit-playsinline
        muted
        class="absolute inset-0 h-full w-full bg-neutral-950 object-cover"
      />

      <!--
        The frame's own shadow paints everything outside it. One element means
        the mask and the border can never drift apart at a different size.
      -->
      <div
        v-if="scanning"
        class="pointer-events-none absolute inset-0"
      >
        <div
          class="absolute top-1/2 left-1/2 aspect-square w-[68%] max-w-80 -translate-x-1/2 -translate-y-1/2 rounded-2xl shadow-[0_0_0_9999px_rgba(0,0,0,0.55)]"
        >
          <span class="absolute -top-0.5 -left-0.5 size-8 rounded-tl-xl border-t-2 border-l-2 border-white/90" />
          <span class="absolute -top-0.5 -right-0.5 size-8 rounded-tr-xl border-t-2 border-r-2 border-white/90" />
          <span class="absolute -bottom-0.5 -left-0.5 size-8 rounded-bl-xl border-b-2 border-l-2 border-white/90" />
          <span class="absolute -right-0.5 -bottom-0.5 size-8 rounded-br-xl border-r-2 border-b-2 border-white/90" />

          <span
            class="animate-scan-sweep absolute inset-x-2 h-0.5 rounded-full bg-sky-300 shadow-[0_0_12px_2px_rgba(56,189,248,0.65)] motion-reduce:animate-none"
          />
        </div>
      </div>

      <p
        v-if="scanning"
        class="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/80 to-transparent px-4 pt-10 pb-4 text-center text-xs text-white/80"
      >
        Point the camera at the payment code
      </p>

      <!--
        Nothing to see until the camera delivers a frame, so cover the panel
        instead of leaving a white card behind a black video element.
      -->
      <div
        v-else
        class="absolute inset-0 grid place-items-center gap-3 bg-neutral-950/85 px-6 text-center"
      >
        <div class="flex flex-col items-center gap-3">
          <UIcon
            :name="message ? 'i-lucide-video-off' : 'i-lucide-loader-circle'"
            :class="['size-8 text-white/60', message ? '' : 'animate-spin']"
          />
          <p class="text-sm text-white/70">
            {{ message || 'Starting the camera…' }}
          </p>
          <UButton
            v-if="message"
            label="Try again"
            size="sm"
            color="neutral"
            variant="solid"
            class="bg-white/15 text-white hover:bg-white/25"
            @click="start"
          />
        </div>
      </div>
    </div>
  </div>
</template>
