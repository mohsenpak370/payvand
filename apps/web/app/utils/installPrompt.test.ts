import { describe, expect, it } from 'vitest'
import { isInstalled, isIos, isIosTab } from './installPrompt'

const IPHONE = 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1'
const IPAD_AS_MAC = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15'
const MAC = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36'
const ANDROID = 'Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Mobile Safari/537.36'

describe('isIos', () => {
  it('recognises an iPhone', () => {
    expect(isIos(IPHONE, 5)).toBe(true)
  })

  it('recognises an iPad that calls itself a Mac, by its touch points', () => {
    expect(isIos(IPAD_AS_MAC, 5)).toBe(true)
  })

  it('leaves a Mac alone', () => {
    // A Mac with touch points does not exist, and would be an iPad in desktop
    // mode anyway — the UA cannot tell them apart, so neither does this.
    expect(isIos(MAC, 0)).toBe(false)
  })

  it('leaves Android alone', () => {
    expect(isIos(ANDROID, 5)).toBe(false)
  })
})

describe('isInstalled', () => {
  const never = () => false
  const standaloneMode = (query: string) => query === '(display-mode: standalone)'

  it('trusts the display mode on any engine', () => {
    expect(isInstalled(undefined, standaloneMode)).toBe(true)
    expect(isInstalled(false, never)).toBe(false)
  })

  it('trusts Safari’s own flag', () => {
    expect(isInstalled(true, never)).toBe(true)
  })
})

describe('isIosTab', () => {
  it('is true on an iPhone in a tab', () => {
    expect(isIosTab(IPHONE, 5, false)).toBe(true)
  })

  it('is false once the Home Screen icon is what opened it', () => {
    expect(isIosTab(IPHONE, 5, true)).toBe(false)
  })

  it('is false everywhere else, where push needs no install', () => {
    expect(isIosTab(ANDROID, 5, false)).toBe(false)
    expect(isIosTab(MAC, 0, false)).toBe(false)
  })
})
