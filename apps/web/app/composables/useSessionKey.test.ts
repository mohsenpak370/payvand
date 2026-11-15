import { address } from '@vue-solana/nuxt/kit'
import { beforeAll, describe, expect, it } from 'vitest'
import { ensureSessionSigner, forgetSessionSigner, peekSessionSigner } from './useSessionKey'

/**
 * Rotating an allowance onto this device reuses the key it already holds
 * instead of minting a fresh one, which is why Task 4.1 has no old key to sweep
 * on this side: the key being replaced belongs to the other device, and this
 * device's own float — the one left behind if it had been rotated out earlier —
 * goes back to paying rather than needing recovery.
 *
 * That reasoning rests on one property: this device answers with the same key
 * every time. Mint a fresh one on rotation and the old key's float is stranded
 * here with no budget naming it, and `open_budget` buys a second float on top.
 * The second test is the other half of the guarantee — forgetting is what
 * actually drops a key, so a sweep has to land before it.
 *
 * The composable leans on Nuxt's auto-imports and browser storage; `appT` is
 * stubbed, and in Node the key store falls back to memory, which is the path a
 * private-browsing session takes too.
 */

beforeAll(() => {
  ;(globalThis as { appT?: (key: string) => string }).appT = (key: string) => key
})

const buyer = address('7Ytt1gCkR5NuMVV6sHYrC3Wt8UwdgVspZGJXjLcU4Vqy')
const otherBuyer = address('FvFg6DToUDBsjsTxdoPEqEMU1WFNGyp3REvjzdkJ5vyj')

describe('ensureSessionSigner', () => {
  it('hands this device the same key every time, so a rotation replaces only the other device', async () => {
    const first = await ensureSessionSigner(buyer)
    const again = await ensureSessionSigner(buyer)

    expect(again.address).toBe(first.address)
    expect((await peekSessionSigner(buyer))?.address).toBe(first.address)
  })

  it('makes a new key only after the old one is forgotten — which is why the sweep goes first', async () => {
    const before = await ensureSessionSigner(otherBuyer)
    forgetSessionSigner(otherBuyer)

    // The forgotten key cannot sign anything again: its float, if any, is out of
    // reach from here. `handBackFloat` sweeps before this point for that reason.
    expect(await peekSessionSigner(otherBuyer)).toBeNull()

    const after = await ensureSessionSigner(otherBuyer)
    expect(after.address).not.toBe(before.address)
  })
})
