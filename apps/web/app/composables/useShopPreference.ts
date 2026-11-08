/**
 * Whether shops belong in this person's app. Plenty of sellers charge for one
 * thing at a time and never want a catalogue hanging off their profile, so the
 * feature can be switched off; that takes its link out of the navigation.
 *
 * It is a browser preference like the currency, not an account setting: it
 * follows this device.
 */
export const SHOPS_STORAGE_KEY = 'payvand.shops-enabled.v1'

export function useShopPreference() {
  // On by default: shops were part of the app before this setting existed, so
  // nobody loses the feature by visiting a page that has not stored a choice.
  const shopsEnabled = useState<boolean>('shops-enabled', () => true)

  function setShopsEnabled(next: boolean) {
    shopsEnabled.value = next
    try {
      localStorage.setItem(SHOPS_STORAGE_KEY, next ? 'on' : 'off')
    } catch {
      // Private mode without storage: the choice still holds for this visit.
    }
  }

  return { shopsEnabled, setShopsEnabled }
}
