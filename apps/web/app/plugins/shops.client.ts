import { SHOPS_STORAGE_KEY } from '~/composables/useShopPreference'

/**
 * Restores the choice made on the settings page. It runs before the first
 * render so the navigation does not paint a Shops link and then take it away.
 */
export default defineNuxtPlugin(() => {
  const shopsEnabled = useState<boolean>('shops-enabled', () => true)
  try {
    const stored = localStorage.getItem(SHOPS_STORAGE_KEY)
    if (stored === 'on') shopsEnabled.value = true
    else if (stored === 'off') shopsEnabled.value = false
  } catch {
    // No storage (private mode, embedded webview): keep the default.
  }
})
