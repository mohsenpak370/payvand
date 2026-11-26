/**
 * The two sounds a payment makes: a coin dropping when it goes through, and a
 * low womp when it does not.
 *
 * They are the WAV files in public/sounds, decoded into an AudioBuffer once and
 * played back through one shared AudioContext. That context is the gesture a
 * browser needs before it will let audio play at all, so it is resumed on the
 * same tap that sends the payment; the files are decoded ahead of time, which
 * keeps the sound from waiting on a fetch that starts inside the click.
 *
 * Whether they play is a browser preference like the currency: it follows this
 * device and is on until someone turns it off in settings.
 */
export const SOUND_STORAGE_KEY = 'payvand.sounds.v1'

const SOURCES = {
  success: '/sounds/success-coin.wav',
  failure: '/sounds/failure-womp.wav'
} as const

type SoundName = keyof typeof SOURCES

/**
 * The files are peak-normalised, and a short percussive hit reads much louder
 * than the steady tones this replaced at the same peak, so it plays well below
 * full scale.
 */
const LEVEL = 0.45

/** One context for every sound: browsers cap how many a page may create. */
let context: AudioContext | null = null

function audio(): AudioContext | null {
  if (!import.meta.client) return null
  if (!context) {
    const Ctor = window.AudioContext
      // Safari before 14 shipped it prefixed, and some webviews still do.
      ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext
    if (!Ctor) return null
    context = new Ctor()
  }
  // Muted until a gesture happened; the payment tap is usually the first one.
  if (context.state === 'suspended') void context.resume()
  return context
}

/** Decoded once per file; the promise is cached so two payments share one fetch. */
const decoded = new Map<SoundName, Promise<AudioBuffer | null>>()

function load(ctx: AudioContext, name: SoundName): Promise<AudioBuffer | null> {
  const cached = decoded.get(name)
  if (cached) return cached
  const pending = fetch(SOURCES[name])
    .then((response) => {
      if (!response.ok) throw new Error(`sound ${name}: ${response.status}`)
      return response.arrayBuffer()
    })
    .then(bytes => ctx.decodeAudioData(bytes))
    .catch(() => null)
  decoded.set(name, pending)
  return pending
}

/**
 * Both files decoded before the first payment needs them. Safe to call without
 * a gesture: decoding works on a suspended context, only playback waits.
 */
export function preloadTransactionSounds() {
  const ctx = audio()
  if (!ctx) return
  for (const name of Object.keys(SOURCES) as SoundName[]) void load(ctx, name)
}

export function useTransactionSounds() {
  const enabled = useState<boolean>('sounds-enabled', () => true)

  function setEnabled(next: boolean) {
    enabled.value = next
    try {
      localStorage.setItem(SOUND_STORAGE_KEY, next ? 'on' : 'off')
    } catch {
      // Private mode without storage: the choice still holds for this visit.
    }
  }

  async function play(name: SoundName) {
    if (!enabled.value) return
    const ctx = audio()
    if (!ctx) return
    const buffer = await load(ctx, name)
    // Turned off, or unmounted into a context that closed, while decoding.
    if (!buffer || !enabled.value || ctx.state === 'closed') return
    const source = ctx.createBufferSource()
    const amp = ctx.createGain()
    amp.gain.value = LEVEL
    source.buffer = buffer
    source.connect(amp)
    amp.connect(ctx.destination)
    source.start()
  }

  /** A coin dropping — a payment settled. */
  function playSuccess() {
    void play('success')
  }

  /** The womp — softer than a buzz, still a no. */
  function playFailure() {
    void play('failure')
  }

  return { enabled, setEnabled, playSuccess, playFailure }
}
