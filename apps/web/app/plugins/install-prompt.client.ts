import { listenForInstall } from '~/composables/useInstallPrompt'

/**
 * `beforeinstallprompt` fires once, early, and an event nobody kept is gone, so
 * the listener is wired here rather than on whichever page offers the install —
 * by the time the settings page mounts, the offer has already been made.
 */
export default defineNuxtPlugin(() => {
  listenForInstall()
})
