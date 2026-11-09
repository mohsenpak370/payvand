/**
 * Whether shops belong in this person's app. Plenty of sellers charge for one
 * thing at a time and never want a catalogue hanging off their profile, so the
 * feature starts switched off and its link stays out of the navigation until it
 * is turned on.
 *
 * It is a browser preference like the currency, not an account setting: it
 * follows this device.
 */
export const SHOPS_STORAGE_KEY = 'payvand.shops-enabled.v1'

export function useShopPreference() {
  // Off by default: most sellers charge for one thing at a time, so a catalogue
  // is something they switch on in settings rather than something they have to
  // find and switch off. Turning it on adds the Shops link and the option to
  // price a payment from a menu.
  const shopsEnabled = useState<boolean>('shops-enabled', () => false)

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
