/**
 * Whether this device can install Payvand, and whether it has to.
 *
 * Kept pure and separate from the browser so the awkward cases can be tested:
 * iPadOS reports itself as a Mac, and a tab opened from the Home Screen looks
 * like a tab until you ask the display mode.
 */

/** Chromium's install offer, which the DOM typings do not include. */
export interface BeforeInstallPromptEvent extends Event {
  prompt: () => Promise<void>
  userChoice: Promise<{ outcome: 'accepted' | 'dismissed', platform: string }>
}

/**
 * iPhone and iPad — including iPadOS 13 and later, which call themselves a Mac.
 * The touch points are what give those away: a Mac has none.
 */
export function isIos(userAgent: string, maxTouchPoints: number): boolean {
  return /iPhone|iPad|iPod/i.test(userAgent)
    || (/Macintosh/i.test(userAgent) && maxTouchPoints > 1)
}

/**
 * True when this window is the installed app rather than a browser tab. Safari
 * answers with `navigator.standalone`; every other engine answers through the
 * display mode.
 */
export function isInstalled(
  standalone: boolean | undefined,
  matchesDisplayMode: (query: string) => boolean
): boolean {
  return standalone === true || matchesDisplayMode('(display-mode: standalone)')
}

/**
 * An iPhone or iPad whose Payvand is still a browser tab. iOS only gives push
 * to an app opened from the Home Screen, so there is nothing to switch on here
 * until it is installed.
 */
export function isIosTab(
  userAgent: string,
  maxTouchPoints: number,
  installed: boolean
): boolean {
  return isIos(userAgent, maxTouchPoints) && !installed
}
