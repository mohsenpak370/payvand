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
 * A payment can also settle in a tab nobody is touching: the buyer taps "open
 * in wallet", pays in that app and comes back. The sheet unlocks the context on
 * its first tap or key press, and a sound the browser refused before that is
 * held for the next one instead of being dropped, so the coin still lands on
 * the page rather than only in the wallet app. The hold belongs to the screen
 * that earned it — the checkout drops it when its result is dismissed or it is
 * left — so a refused coin can never ring from a tap on another page.
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

/**
 * How long a refused sound waits for the page to be touched. A coin a minute
 * after the payment is still the payment's sound; one played out of a tap much
 * later than that would just be noise.
 */
const PENDING_MS = 60_000

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
 * A sound the browser would not start yet, waiting for a gesture it can play
 * from. Only one is kept: a second settlement replaces the first rather than
 * queueing a coin behind a womp.
 */
let pendingSound: { buffer: AudioBuffer, until: number } | null = null

/**
 * Drops a held sound. The page that deferred it calls this when the result it
 * belongs to is gone, so the next tap elsewhere cannot play it.
 */
export function discardPendingTransactionSound() {
  pendingSound = null
}

function playBuffer(ctx: AudioContext, buffer: AudioBuffer) {
  const source = ctx.createBufferSource()
  const amp = ctx.createGain()
  amp.gain.value = LEVEL
  source.buffer = buffer
  source.connect(amp)
  amp.connect(ctx.destination)
  source.start()
}

function flushPendingSound(ctx: AudioContext) {
  if (!pendingSound || ctx.state !== 'running') return
  const { buffer, until } = pendingSound
  pendingSound = null
  if (Date.now() > until) return
  playBuffer(ctx, buffer)
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

/** Whether the one-time unlock listeners are still attached. */
let unlocking = false

/**
 * Resumes the context on the page's first tap, touch or key press, and plays
 * whatever it refused before that. Safari only starts an AudioContext from
 * inside a gesture handler, so a settlement that arrives afterwards — from the
 * poll, with no tap of its own — needs the context to have been resumed while
 * the buyer was still here.
 *
 * The listeners remove themselves once the context runs. Calling this again is
 * a no-op while they wait.
 */
export function unlockTransactionSounds() {
  if (!import.meta.client || unlocking) return
  unlocking = true
  const types = ['pointerdown', 'touchstart', 'keydown'] as const
  const unlock = () => {
    const ctx = audio()
    if (!ctx) return
    void ctx.resume().then(() => {
      // Still refused: this gesture was not enough, so the listeners stay for
      // the next one.
      if (ctx.state !== 'running') return
      for (const type of types) window.removeEventListener(type, unlock, true)
      unlocking = false
      flushPendingSound(ctx)
    })
  }
  for (const type of types) window.addEventListener(type, unlock, true)
}

export function useTransactionSounds() {
  const enabled = useState<boolean>('sounds-enabled', () => true)

  function setEnabled(next: boolean) {
    enabled.value = next
    // Turning sounds off drops one already held for the next tap.
    if (!next) pendingSound = null
    try {
      localStorage.setItem(SOUND_STORAGE_KEY, next ? 'on' : 'off')
    } catch {
      // Private mode without storage: the choice still holds for this visit.
    }
  }

  /**
   * The hold lives in this module, but it belongs to a screen: dropping it when
   * that screen unmounts is what keeps a refused coin from following the buyer
   * to the next page.
   */
  if (getCurrentScope()) onScopeDispose(discardPendingTransactionSound)

  async function play(name: SoundName) {
    if (!enabled.value) return
    const ctx = audio()
    if (!ctx) return
    const buffer = await load(ctx, name)
    // Turned off, or unmounted into a context that closed, while decoding.
    if (!buffer || !enabled.value) return
    // A suspended context means no gesture has reached the page yet. Resuming
    // is allowed once the buyer has tapped anything, so try that first and keep
    // the sound for the next tap only when the browser still refuses.
    if (ctx.state !== 'running') {
      await ctx.resume().catch(() => {})
      if (!enabled.value) return
    }
    // Read after the resume: the state this started with no longer describes it.
    const state: AudioContextState = ctx.state
    if (state === 'closed') return
    if (state !== 'running') {
      pendingSound = { buffer, until: Date.now() + PENDING_MS }
      return
    }
    playBuffer(ctx, buffer)
  }

  /** A coin dropping — a payment settled. */
  function playSuccess() {
    void play('success')
  }

  /** The womp — softer than a buzz, still a no. */
  function playFailure() {
    void play('failure')
  }

  return { enabled, setEnabled, playSuccess, playFailure, discardPending: discardPendingTransactionSound }
}
