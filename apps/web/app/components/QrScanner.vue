<script setup lang="ts">
const emit = defineEmits<{
  decoded: [text: string]
}>()

const { t } = useI18n()

const videoRef = ref<HTMLVideoElement>()
const message = ref('')
const scanning = ref(false)
/** A frame with real dimensions has arrived, so the panel can show the camera. */
const framesFlowing = ref(false)
/** Playback would not start on its own and needs a tap from the buyer. */
const playBlocked = ref(false)
/** Something to say under the viewfinder after a while without a code. */
const hint = ref('')

let stream: MediaStream | null = null
let waitNotice = 0
let playWatchdog = 0
let frame = 0
let canvas: HTMLCanvasElement | null = null
let context: CanvasRenderingContext2D | null = null
let cameraLabel = ''
let startedAt = 0
let reads = 0

/*
 * The browser's own decoder goes first — the same kind the scanner apps use —
 * and rxing (a Rust port of ZXing, compiled to WebAssembly) reads the frames it
 * cannot. The codes this page meets are dense: the whole invoice rides in the
 * link, so a seller offering two currencies has a hundred and forty modules in
 * a code a screen shows a few hundred pixels wide. Measured on a lens-softened
 * camera frame of exactly that code, the wasm reader resolves it down to about
 * two pixels per module in a few milliseconds; the JavaScript reader it
 * replaced (jsQR) failed at every size the app shows, even above three pixels
 * per module, which is why a phone without a native decoder never scanned.
 */
interface BarcodeDetectorLike {
  detect: (source: CanvasImageSource) => Promise<{ rawValue: string }[]>
}

type BarcodeDetectorCtor = {
  new (options: { formats: string[] }): BarcodeDetectorLike
  getSupportedFormats?: () => Promise<string[]>
}

let detector: BarcodeDetectorLike | null = null
let detectorFailures = 0

/**
 * The interface existing says nothing about the device behind it: Chrome
 * exposes `BarcodeDetector` on Android phones whose Play Services cannot
 * decode a single code, and constructing one succeeds either way. Chrome's own
 * guidance is to ask what the platform supports, which is the only way to tell
 * — otherwise such a phone looks native-capable and the wasm reader, the one
 * that could have read the code, never runs.
 */
async function createDetector(): Promise<BarcodeDetectorLike | null> {
  const ctor = (window as unknown as { BarcodeDetector?: BarcodeDetectorCtor }).BarcodeDetector
  if (!ctor) return null
  try {
    if (ctor.getSupportedFormats) {
      const formats = await ctor.getSupportedFormats()
      if (!formats.includes('qr_code')) return null
    }
    return new ctor({ formats: ['qr_code'] })
  } catch {
    return null
  }
}

async function start() {
  if (scanning.value) return
  message.value = ''
  hint.value = ''
  clearTimeout(waitNotice)
  clearTimeout(playWatchdog)

  // Browsers only expose the camera in a secure context: over plain
  // http:// on a LAN address `mediaDevices` doesn't exist at all, and the
  // permission prompt will never appear. Say so instead of pretending the
  // user denied something.
  if (!navigator.mediaDevices?.getUserMedia) {
    message.value = t('scanner.insecure')
    return
  }

  /*
   * A prompt nobody answers leaves the request pending forever, and the panel
   * would sit on "Starting the camera…" for as long as the tab is open. Say
   * what to do about it instead.
   */
  waitNotice = window.setTimeout(() => {
    if (!scanning.value) {
      message.value = t('scanner.notStarted')
    }
  }, 15_000)

  try {
    // `ideal` rather than a hard requirement: on a laptop there is no back
    // camera, and asking for one as a requirement fails the whole request.
    //
    // The size is asked for explicitly for the same reason: left to itself a
    // camera hands back 640×480, where a dense code is a couple of pixels per
    // module and nothing can read it. The device still picks its own default
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
    message.value = t('scanner.denied')
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

  // What the camera actually gave us, so a hint can say it outright.
  const settings = track?.getSettings() ?? {}
  cameraLabel = settings.width && settings.height ? `${settings.width}×${settings.height}` : ''

  const video = videoRef.value
  if (!video) return

  canvas = document.createElement('canvas')
  context = canvas.getContext('2d', { willReadFrequently: true })
  detector = await createDetector()
  detectorFailures = 0
  reads = 0
  framesFlowing.value = false
  playBlocked.value = false
  video.srcObject = stream
  startedAt = performance.now()
  scanning.value = true

  /*
   * Playback is not always granted without a gesture — Android browsers and
   * iOS both refuse sometimes — and a refused `play()` used to be swallowed,
   * which left a black panel that never scanned anything. Ask, and if the
   * browser says no, put a button on screen that asks again on a real tap.
   */
  void video.play().then(
    () => {
      playBlocked.value = false
    },
    () => {
      playBlocked.value = true
    }
  )
  playWatchdog = window.setTimeout(() => {
    if (scanning.value && !framesFlowing.value) playBlocked.value = true
  }, 2_500)

  tick(performance.now())
}

/** Start playback again, from a tap the browser will honour. */
async function resume() {
  const video = videoRef.value
  if (!video) return
  try {
    await video.play()
    playBlocked.value = false
  } catch {
    playBlocked.value = true
  }
}

/**
 * Twelve reads a second is plenty to catch a code someone is holding still,
 * and the loop only asks for the next frame once a read has finished, so a slow
 * decoder throttles itself instead of piling work up.
 */
const SCAN_INTERVAL_MS = 80
let lastScan = 0

async function tick(now: number) {
  if (!scanning.value) return

  const video = videoRef.value
  if (video) {
    if (video.videoWidth > 0) {
      if (!framesFlowing.value) {
        framesFlowing.value = true
        playBlocked.value = false
        clearTimeout(playWatchdog)
      }
    } else if (now - startedAt > 3_000) {
      playBlocked.value = true
    }
  }

  if (video && context && canvas && video.readyState === video.HAVE_ENOUGH_DATA && now - lastScan >= SCAN_INTERVAL_MS) {
    lastScan = now
    try {
      const text = await readFrame(video, now)
      if (text) {
        stop()
        emit('decoded', text)
        return
      }
      if (!hint.value && performance.now() - startedAt > 8_000) {
        /*
         * Nothing has been read. When the wasm reader could not even start
         * there is nothing left to try, and saying so beats letting the buyer
         * keep waving the phone; otherwise the code is simply too small in
         * the frame, which is the one thing they can fix.
         */
        hint.value = rxingUnavailable
          ? t('scanner.noDecoder')
          : cameraLabel
            ? t('scanner.noCodeWithCamera', { camera: cameraLabel })
            : t('scanner.noCode')
      }
    } catch {
      // A frame that cannot be read is no reason to give up the camera.
    }
  }

  frame = requestAnimationFrame(tick)
}

async function readFrame(video: HTMLVideoElement, now: number): Promise<string | null> {
  const image = scanImage(video)
  if (!image) return null
  reads += 1

  if (detector) {
    try {
      const [found] = await detector.detect(image)
      if (found?.rawValue) return found.rawValue
    } catch {
      // The detector can refuse a frame while the camera is still warming up;
      // if it keeps refusing, it is not usable here and the wasm reader takes
      // over.
      if (++detectorFailures >= 3) detector = null
    }
  }

  /*
   * The wasm reader is 2.4 MB, which no one should pay for on a phone that can
   * read the code by itself — so it is fetched only when it is needed: on a
   * browser without a native decoder, or after a native one has had a few
   * seconds and found nothing.
   */
  const needed = !detector || now - startedAt > RXING_AFTER_MS
  if (!needed) return null
  // Once both are in play, the wasm reader takes every other read.
  if (detector && reads % 2 === 1) return null

  return decodeWithRxing(image)
}

/** How long a native decoder gets on its own before the wasm reader joins in. */
const RXING_AFTER_MS = 3_000

let rxing: typeof import('rxing-wasm') | null = null
let rxingUnavailable = false

/**
 * rxing is a Rust port of ZXing compiled to WebAssembly: the same decoder the
 * phone in a scanner app uses, and the only thing that reads these codes on a
 * browser whose own detector is missing or refuses them.
 */
async function decodeWithRxing(image: HTMLCanvasElement): Promise<string | null> {
  if (rxingUnavailable || !context) return null

  if (!rxing) {
    try {
      rxing = await import('rxing-wasm')
    } catch {
      // A browser that cannot start the module cannot be helped further.
      rxingUnavailable = true
      return null
    }
  }

  try {
    const pixels = context.getImageData(0, 0, image.width, image.height)
    const luma = rxing.convert_imagedata_to_luma(pixels)
    // `true` is rxing's try-harder: accuracy over speed, and at a few
    // milliseconds a frame there is plenty of speed to give away.
    return rxing.decode_barcode(luma, pixels.width, pixels.height, true).text()
  } catch {
    // "not found" is the ordinary answer for a frame without a code in it.
    return null
  }
}

/** Smallest crop worth decoding: a camera that hands back less is upscaled. */
const SCAN_MIN_PX = 900
/** Cap, so a read stays affordable on a phone. */
const SCAN_MAX_PX = 1200

/**
 * What both decoders read: the square in the middle of the frame — the region
 * the viewfinder points at, and, because the video is drawn to cover its box,
 * exactly the part the buyer can see. It is never shrunk below the camera's own
 * sampling, since that is the detail a dense code depends on.
 */
function scanImage(video: HTMLVideoElement): HTMLCanvasElement | null {
  if (!canvas || !context) return null

  const side = Math.min(video.videoWidth, video.videoHeight)
  if (side < 32) return null

  const size = Math.min(SCAN_MAX_PX, Math.max(SCAN_MIN_PX, side))
  canvas.width = size
  canvas.height = size
  // Nearest-neighbour on purpose: a smooth resample blurs the module edges the
  // readers are looking for.
  context.imageSmoothingEnabled = false
  context.drawImage(
    video,
    Math.round((video.videoWidth - side) / 2),
    Math.round((video.videoHeight - side) / 2),
    side,
    side,
    0,
    0,
    size,
    size
  )

  return canvas
}

function stop() {
  scanning.value = false
  framesFlowing.value = false
  clearTimeout(waitNotice)
  clearTimeout(playWatchdog)
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
        v-if="scanning && framesFlowing"
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
        v-if="scanning && framesFlowing"
        class="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/80 to-transparent px-4 pt-10 pb-4 text-center text-xs text-white/80"
      >
        {{ hint || $t('scanner.pointCamera') }}
      </p>

      <!--
        Nothing to see until the camera delivers a frame, so cover the panel
        instead of leaving a white card behind a black video element. When
        playback itself was refused, the cover carries the tap that starts it.
      -->
      <div
        v-else
        class="absolute inset-0 grid place-items-center gap-3 bg-neutral-950/85 px-6 text-center"
      >
        <div class="flex flex-col items-center gap-3">
          <UIcon
            :name="message || playBlocked ? 'i-lucide-video-off' : 'i-lucide-loader-circle'"
            :class="['size-8 text-white/60', message || playBlocked ? '' : 'animate-spin']"
          />
          <p class="text-sm text-white/70">
            {{ message || (playBlocked ? $t('scanner.waitingTap') : $t('scanner.starting')) }}
          </p>
          <UButton
            v-if="message"
            :label="$t('scanner.tryAgain')"
            size="sm"
            color="neutral"
            variant="solid"
            class="bg-white/15 text-white hover:bg-white/25"
            @click="start"
          />
          <UButton
            v-else-if="playBlocked"
            :label="$t('scanner.startCamera')"
            size="sm"
            color="neutral"
            variant="solid"
            class="bg-white/15 text-white hover:bg-white/25"
            @click="resume"
          />
        </div>
      </div>
    </div>
  </div>
</template>
