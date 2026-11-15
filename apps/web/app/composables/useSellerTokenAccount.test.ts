import { address } from '@vue-solana/nuxt/kit'
import { describe, expect, it } from 'vitest'
import { ASSOCIATED_TOKEN_PROGRAM_ID, TOKEN_PROGRAM_ID } from './usePaymentProgram'
import { openTokenAccountInstruction } from './useSellerTokenAccount'

/**
 * The one instruction the app builds by hand, so its layout is pinned here:
 * the associated-token program's `CreateIdempotent` takes payer, account,
 * owner, mint, system program, token program in that order, and a wrong order
 * fails at payment time rather than at build time.
 */
describe('openTokenAccountInstruction', () => {
  const owner = address('9WMT ojUUWWn4dhuqrS6zqqP8s1X1FSgg7u7DGNwGAMAT'.replace(' ', ''))
  const mint = address('4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU')
  const tokenAccount = address('FzXSgvDiVQqQMTwKZ5VwGxVgcAvU5QqZ3q8Z1MRyS7iB')
  const instruction = openTokenAccountInstruction(owner, mint, tokenAccount)

  it('targets the associated-token program with CreateIdempotent', () => {
    expect(instruction.programAddress).toBe(ASSOCIATED_TOKEN_PROGRAM_ID)
    expect([...instruction.data!]).toEqual([1])
  })

  it('passes payer, account, owner, mint, system and token program in order', () => {
    const accounts = instruction.accounts!.map(account => ({
      address: account.address,
      role: account.role
    }))

    expect(accounts).toEqual([
      { address: owner, role: 3 },
      { address: tokenAccount, role: 1 },
      { address: owner, role: 0 },
      { address: mint, role: 0 },
      { address: '11111111111111111111111111111111', role: 0 },
      { address: TOKEN_PROGRAM_ID, role: 0 }
    ])
  })
})
