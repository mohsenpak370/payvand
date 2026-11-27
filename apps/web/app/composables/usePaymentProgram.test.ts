import { address, type Address } from '@vue-solana/nuxt/kit'
import { describe, expect, it } from 'vitest'
import idl from '~/assets/payvand.idl.json'
import {
  COMPUTE_BUDGET_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  buildInstruction,
  computeUnitLimitInstruction
} from './usePaymentProgram'

/**
 * The settle paths take no optional account since the per-shop fee feature was
 * removed (plan 0005): the IDL's list is the whole instruction, with `sponsor`
 * last. The builder still stops at an account the IDL marks optional — Anchor's
 * trailing-optional rule, kept for any future one — and refuses a missing
 * account that is not.
 *
 * The builder is what decides the account list the app sends, so those two rules
 * are what keep a built settle shippable.
 */

const payer = address('3rpBzUBbZPXFKhGywCRduD3hrREDjvDtaw2Mos2yUUPx')
const seller = address('6dQgFZDTkhDMTZv9ftZe4ufU9jG5fvJEw6zpw6WCKjPz')
const reference = address('A8yxpWod2xrPd6YbXnaKWCTkk2WduzQKdKPUANZFQpft')
const elsewhere = reference

/** The eight accounts `settle_payment` needs before the sponsor. */
function settleAccounts() {
  return {
    payer,
    payment: elsewhere,
    seller,
    receipt: elsewhere,
    reference,
    config: elsewhere,
    treasury: elsewhere,
    system_program: address('11111111111111111111111111111111'),
    sponsor: payer
  }
}

describe('computeUnitLimitInstruction', () => {
  /**
   * A sponsored settle carries this so the wallet adds no budget of its own:
   * Phantom appends a compute-unit limit *and* a priority price to any
   * transaction that has neither, and that price is the relayer's money. The
   * bytes are the program's: discriminator 2, then the limit as a `u32`.
   */
  it('asks for 200,000 units and names no account', () => {
    const instruction = computeUnitLimitInstruction()

    expect(instruction.programAddress).toBe(COMPUTE_BUDGET_PROGRAM_ID)
    expect(instruction.accounts).toEqual([])
    expect(Array.from(instruction.data!)).toEqual([2, 0x40, 0x0d, 0x03, 0x00])
  })
})

describe('buildInstruction', () => {
  it('builds the settle account list in the IDL order, sponsor last', () => {
    const instruction = buildInstruction('settle_payment', { amount: 1, memo: '' }, settleAccounts())

    // The program declares nine accounts for a SOL settle, and `sponsor` is the
    // last of them.
    expect(instruction.accounts).toHaveLength(9)
    expect(instruction.accounts!.at(-1)?.address).toBe(settleAccounts().sponsor)
  })

  it('still refuses a missing account the IDL does not mark optional', () => {
    const { treasury: _treasury, ...withoutTreasury } = settleAccounts()

    expect(() => buildInstruction('settle_payment', { amount: 1, memo: '' }, withoutTreasury))
      .toThrow('Missing account "treasury" for settle_payment')
  })

  /**
   * `token_program` is the one account the IDL cannot give an address: the
   * program takes the token program as an interface, and Anchor writes no
   * address for those. Every token instruction has to get it from the builder,
   * or it refuses to be built — which is what "Missing account \"token_program\"
   * for pay_from_token_budget" was.
   */
  it('fills in the token program the IDL leaves nameless', () => {
    const instructions = (idl as { instructions: {
      name: string
      args: { name: string, type: string }[]
      accounts: { name: string, optional?: boolean, address?: string }[]
    }[] }).instructions

    const tokenInstructions = instructions.filter(instruction =>
      instruction.accounts.some(account => account.name === 'token_program' && !account.address)
    )
    expect(tokenInstructions.length).toBeGreaterThan(0)

    for (const instruction of tokenInstructions) {
      const accounts: Record<string, Address> = {}
      for (const account of instruction.accounts) {
        if (account.name === 'token_program') continue
        // An optional account is trailing: stopping here is "none", as the
        // program reads the shorter list.
        if (account.optional) break
        accounts[account.name] = payer
      }
      const args = Object.fromEntries(
        instruction.args.map(arg => [arg.name, arg.type === 'string' ? '' : 1])
      )

      const built = buildInstruction(instruction.name, args, accounts)
      const index = instruction.accounts.findIndex(account => account.name === 'token_program')

      expect(built.accounts![index]?.address, instruction.name).toBe(TOKEN_PROGRAM_ID)
    }
  })
})
