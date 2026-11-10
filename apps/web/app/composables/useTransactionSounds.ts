/**
 * The two sounds a payment makes: a short rise when it goes through, and a low
 * double buzz when it does not.
 *
 * They are synthesised with WebAudio rather than shipped as files. Two tones
 * cost less than a download, and an oscillator can start on the same tap that
 * sends the payment — which is the gesture a browser needs before it will let
 * audio play at all.
 *
 * Whether they play is a browser preference like the currency: it follows this
 * device and is on until someone turns it off in settings.
 */
export const SOUND_STORAGE_KEY = 'payvand.sounds.v1'

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

function note(
  ctx: AudioContext,
  frequency: number,
  at: number,
  duration: number,
  type: OscillatorType = 'sine'
) {
  const oscillator = ctx.createOscillator()
  const amp = ctx.createGain()
  oscillator.type = type
  oscillator.frequency.setValueAtTime(frequency, at)
  // Ramp in and out: a tone that starts or stops abruptly clicks.
  amp.gain.setValueAtTime(0, at)
  amp.gain.linearRampToValueAtTime(0.09, at + 0.012)
  amp.gain.setValueAtTime(0.09, at + Math.max(duration - 0.05, 0.014))
  amp.gain.linearRampToValueAtTime(0, at + duration)
  oscillator.connect(amp)
  amp.connect(ctx.destination)
  oscillator.start(at)
  oscillator.stop(at + duration + 0.02)
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

  /** Two notes up, an octave apart — a payment settled. */
  function playSuccess() {
    if (!enabled.value) return
    const ctx = audio()
    if (!ctx) return
    const now = ctx.currentTime
    note(ctx, 784, now, 0.12) // G5
    note(ctx, 1175, now + 0.1, 0.18) // D6
  }

  /** Two notes down on a triangle wave — softer than a square, still a no. */
  function playFailure() {
    if (!enabled.value) return
    const ctx = audio()
    if (!ctx) return
    const now = ctx.currentTime
    note(ctx, 233, now, 0.16, 'triangle')
    note(ctx, 175, now + 0.14, 0.24, 'triangle')
  }

  return { enabled, setEnabled, playSuccess, playFailure }
}
