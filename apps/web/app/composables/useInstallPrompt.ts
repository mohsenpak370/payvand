import type { BeforeInstallPromptEvent } from '~/utils/installPrompt'
import { isInstalled } from '~/utils/installPrompt'

/**
 * Chromium's install offer, held until the person asks for it.
 *
 * `beforeinstallprompt` fires once per page load, usually long before the
 * settings page mounts, so a plugin listens from the first paint and keeps the
 * event here rather than inside a component. The browser accepts one `prompt()`
 * per event, so the offer is dropped the moment it is used: accepted or
 * dismissed, it cannot be shown again.
 *
 * Safari (macOS and iOS), Firefox and in-app browsers never fire the event, so
 * there `canInstall` stays false and no prompt is shown — iOS gets its own
 * instructions on the notifications card instead.
 */
const deferred = shallowRef<BeforeInstallPromptEvent | null>(null)
const installed = ref(false)
let listening = false

/**
 * Wired once, from `app/plugins/install-prompt.client.ts` — early enough to
 * catch an event that fires before the page's components mount.
 */
export function listenForInstall(): void {
  if (!import.meta.client || listening) return
  listening = true
  installed.value = isInstalled(
    (navigator as Navigator & { standalone?: boolean }).standalone,
    query => window.matchMedia(query).matches
  )

  window.addEventListener('beforeinstallprompt', (event) => {
    // Keeps the browser's own mini-infobar away: the offer belongs on the page.
    event.preventDefault()
    deferred.value = event as BeforeInstallPromptEvent
  })

  window.addEventListener('appinstalled', () => {
    deferred.value = null
    installed.value = true
  })
}

export function useInstallPrompt() {
  const canInstall = computed(() => Boolean(deferred.value) && !installed.value)

  /** Opens the browser's install dialog; true when it was accepted. */
  async function install(): Promise<boolean> {
    const event = deferred.value
    if (!event) return false
    deferred.value = null
    await event.prompt()
    const { outcome } = await event.userChoice
    return outcome === 'accepted'
  }

  return { canInstall, installed, install }
}
