/**
 * The mark's background: the dark tile the Payvand logo draws behind its
 * letters, and the colour a QR code's clearing is painted so the middle of the
 * code reads as that tile before the mark is drawn onto it.
 *
 * One value, because the two have to match exactly: the clearing sits under the
 * tile in the hero's code, and a second copy of the hex would only be a way for
 * them to drift apart.
 */
export const MARK_BACKGROUND = '#080e20'
