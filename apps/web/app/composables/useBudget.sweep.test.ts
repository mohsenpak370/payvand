import { address } from '@vue-solana/nuxt/kit'
import { describe, expect, it } from 'vitest'
import { sweepableLamports, transferLamportsInstruction } from './useBudget'

/**
 * Task 4.1 hands the session key's float back before the key is forgotten. The
 * transfer is a plain system-program one — the program cannot make it, because
 * the session key is not a PDA — so the two things worth pinning are the amount
 * it may move and the bytes it moves them with. Getting the amount wrong spends
 * the last lamports the signature needs and the sweep fails; getting the bytes
 * wrong sends the wrong amount somewhere the chain accepts.
 *
 * The sweep is one signature with no priority fee, so the fee is the cluster's
 * base fee — the residue a sweep leaves behind has to be nothing, or the key
 * this device forgets still holds lamports nobody can spend.
 */

const from = address('B8V5fS6rPD9P9Hhw6xYJ2Kf7UEfFQHhBb4HfCxRxJJPb')
const to = address('3rpBzUBbZPXFKhGywCRduD3hrREDjvDtaw2Mos2yUUPx')
/** The cluster's base fee for one signature. */
const BASE_FEE = 5_000

describe('sweepableLamports', () => {
  it('hands back a funded float minus the fee its own signature costs', () => {
    // What `open_budget` leaves on a fresh session key: 0.03 SOL.
    expect(sweepableLamports(30_000_000)).toBe(30_000_000 - BASE_FEE)
    // The key keeps exactly the fee, so the sweep spends its balance to zero.
    expect(30_000_000 - sweepableLamports(30_000_000)).toBe(BASE_FEE)
  })

  it('sweeps nothing from a key that holds only its fee', () => {
    expect(sweepableLamports(BASE_FEE)).toBe(0)
  })

  it('never asks for a negative or impossible transfer', () => {
    // A key worn down to dust, and one already empty: a transfer of 0 is
    // refused by the system program, so neither may produce one.
    expect(sweepableLamports(5_000)).toBe(0)
    expect(sweepableLamports(0)).toBe(0)
  })
})

describe('transferLamportsInstruction', () => {
  it('is the system program transfer, signed by the key that owns the lamports', () => {
    const instruction = transferLamportsInstruction(from, to, 29_990_000)

    expect(instruction.programAddress).toBe('11111111111111111111111111111111')
    expect(instruction.accounts).toEqual([
      { address: from, role: 3 },
      { address: to, role: 1 }
    ])
  })

  it('encodes instruction 2 and the lamports little-endian, as the chain reads them', () => {
    const lamports = 29_990_000
    const data = transferLamportsInstruction(from, to, lamports).data ?? new Uint8Array()
    const view = new DataView(data.buffer, data.byteOffset, data.byteLength)

    expect(data).toHaveLength(12)
    expect(view.getUint32(0, true)).toBe(2)
    expect(view.getBigUint64(4, true)).toBe(BigInt(lamports))
  })
})
