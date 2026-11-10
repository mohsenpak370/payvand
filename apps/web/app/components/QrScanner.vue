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

/*
 * jsQR is a strict grid reader, and the codes this page meets are dense: the
 * whole invoice rides in the link, so a seller offering two currencies has a
 * hundred and forty modules in a code a phone screen shows a few hundred
 * pixels wide. Below roughly three pixels per module it stops resolving them,
 * which is why a camera frame is not enough on its own — the browser's own
 * decoder, the same kind the scanner apps use, copes far better. Prefer it
 * where the browser has it and keep jsQR as the fallback.
 */
interface BarcodeDetectorLike {
  detect: (source: CanvasImageSource) => Promise<{ rawValue: string }[]>
}

type BarcodeDetectorCtor = new (options: { formats: string[] }) => BarcodeDetectorLike

let detector: BarcodeDetectorLike | null = null
let detectorFailures = 0

function createDetector(): BarcodeDetectorLike | null {
  const ctor = (window as unknown as { BarcodeDetector?: BarcodeDetectorCtor }).BarcodeDetector
  if (!ctor) return null
  try {
    return new ctor({ formats: ['qr_code'] })
  } catch {
    // A browser can expose the API and still refuse the format.
    return null
  }
}

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
    //
    // The size is asked for explicitly for the same reason: left to itself a
    // webcam hands back 640×480, where a dense code is a couple of pixels per
    // module and nothing can read it. The camera still picks its own default
    // if it cannot do better, and the decoder copes with what it gets.
    stream = await navigator.mediaDevices.getUserMedia({
      video: {
        facingMode: { ideal: 'environment' },
        width: { ideal: 1920 },
        height: { ideal: 1080 }
      },
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
  detector = createDetector()
  detectorFailures = 0
  video.srcObject = stream
  /*
   * `play()` can stay pending until the page gets a gesture (iOS is the usual
   * offender), and awaiting it meant the panel sat on "Starting the camera…"
   * with a working camera behind it. Let it resolve on its own: `tick()` waits
   * for frames anyway, and replays are silent if the browser refuses.
   */
  void video.play().catch(() => {})
  scanning.value = true
  tick(performance.now())
}

/**
 * Twelve frames a second is plenty to catch a code someone is holding still,
 * and a tenth of what reading a full-resolution frame costs.
 */
const SCAN_INTERVAL_MS = 80
let lastScan = 0

async function tick(now: number) {
  if (!scanning.value) return

  const video = videoRef.value
  if (video && context && canvas && video.readyState === video.HAVE_ENOUGH_DATA && now - lastScan >= SCAN_INTERVAL_MS) {
    lastScan = now
    try {
      const text = await readFrame(video)
      if (text) {
        stop()
        emit('decoded', text)
        return
      }
    } catch {
      // A frame that cannot be read is no reason to give up the camera.
    }
  }

  frame = requestAnimationFrame(tick)
}

async function readFrame(video: HTMLVideoElement): Promise<string | null> {
  if (detector) {
    try {
      const [found] = await detector.detect(video)
      // A frame with no code in it is the normal case, not a failure.
      return found?.rawValue ?? null
    } catch {
      // The detector can refuse a frame while the camera is still warming up;
      // if it keeps refusing, it is not usable here and jsQR takes over.
      if (++detectorFailures >= 3) detector = null
      return null
    }
  }

  return decodeWithJsQr(video)
}

function decodeWithJsQr(video: HTMLVideoElement): string | null {
  if (!canvas || !context) return null

  canvas.width = video.videoWidth
  canvas.height = video.videoHeight
  context.drawImage(video, 0, 0, canvas.width, canvas.height)

  const whole = context.getImageData(0, 0, canvas.width, canvas.height)
  const code = jsQR(whole.data, whole.width, whole.height, {
    inversionAttempts: 'attemptBoth'
  })
  if (code?.data) return code.data

  /*
   * Nothing over the whole frame, so give the middle of it more pixels than the
   * camera did. The viewfinder already asks the buyer to put the code there,
   * and jsQR's binarizer resolves a module far better at four pixels than at
   * two — the frame carries the detail even though the camera sampled it
   * sparsely. Nearest-neighbour on purpose: a smooth resample blurs the edges
   * the reader is looking for.
   */
  const side = Math.round(Math.min(video.videoWidth, video.videoHeight) * 0.8)
  if (side < 32) return null

  const zoomed = Math.min(1024, side * 3)
  context.imageSmoothingEnabled = false
  canvas.width = zoomed
  canvas.height = zoomed
  context.drawImage(
    video,
    Math.round((video.videoWidth - side) / 2),
    Math.round((video.videoHeight - side) / 2),
    side,
    side,
    0,
    0,
    zoomed,
    zoomed
  )

  const centre = context.getImageData(0, 0, zoomed, zoomed)
  return jsQR(centre.data, centre.width, centre.height, {
    inversionAttempts: 'attemptBoth'
  })?.data ?? null
}

function stop() {
  scanning.value = false
  clearTimeout(waitNotice)
  cancelAnimationFrame(frame)
  for (const track of stream?.getTracks() ?? []) track.stop()
  stream = null
  detector = null
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
          <!--
            The brackets sit on the frame's own border box and share its corner
            radius (2xl, the same token), so the white line lands exactly on the
            edge of the undimmed area. A smaller radius or a pixel of offset
            leaves a wedge of gap between the bracket and the scan area.
          -->
          <span class="absolute top-0 left-0 size-8 rounded-tl-2xl border-t-2 border-l-2 border-white/90" />
          <span class="absolute top-0 right-0 size-8 rounded-tr-2xl border-t-2 border-r-2 border-white/90" />
          <span class="absolute bottom-0 left-0 size-8 rounded-bl-2xl border-b-2 border-l-2 border-white/90" />
          <span class="absolute right-0 bottom-0 size-8 rounded-br-2xl border-r-2 border-b-2 border-white/90" />

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
