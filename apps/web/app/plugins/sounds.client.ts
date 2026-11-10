import { SOUND_STORAGE_KEY } from '~/composables/useTransactionSounds'

/**
 * Restores the choice made on the settings page. Sounds are on by default, so
 * only a deliberate "off" needs carrying over to the next visit.
 */
export default defineNuxtPlugin(() => {
  const soundsEnabled = useState<boolean>('sounds-enabled', () => true)
  try {
    const stored = localStorage.getItem(SOUND_STORAGE_KEY)
    if (stored === 'on') soundsEnabled.value = true
    else if (stored === 'off') soundsEnabled.value = false
  } catch {
    // No storage (private mode, embedded webview): keep the default.
  }
})
