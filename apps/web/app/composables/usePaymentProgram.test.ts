import { AccountRole, address, type Address } from '@vue-solana/nuxt/kit'
import { describe, expect, it } from 'vitest'
import idl from '~/assets/payvand.idl.json'
import { TOKEN_PROGRAM_ID, buildInstruction } from './usePaymentProgram'

/**
 * The settle paths name a shop's `ShopPolicy` as their last account and mark it
 * optional in the IDL: the program reads the shorter list as "none", which is
 * what keeps an invoice with no shop — and every QR printed before shops had
 * rates — settling at `Config`'s rate.
 *
 * The builder is what decides the account list the app sends, so stopping at a
 * missing optional account is the whole compatibility rule. Without it every
 * settle the app builds would ask for a `shop_policy` the seller never wrote.
 */

const payer = address('3rpBzUBbZPXFKhGywCRduD3hrREDjvDtaw2Mos2yUUPx')
const seller = address('6dQgFZDTkhDMTZv9ftZe4ufU9jG5fvJEw6zpw6WCKjPz')
const reference = address('A8yxpWod2xrPd6YbXnaKWCTkk2WduzQKdKPUANZFQpft')
const elsewhere = reference

/** The eight accounts `settle_payment` needs besides the optional policy. */
function settleAccounts() {
  return {
    payer,
    payment: elsewhere,
    seller,
    receipt: elsewhere,
    reference,
    config: elsewhere,
    treasury: elsewhere,
    system_program: address('11111111111111111111111111111111')
  }
}

describe('buildInstruction', () => {
  it('ends the account list at the optional shop policy', () => {
    const instruction = buildInstruction('settle_payment', { amount: 1, memo: '' }, settleAccounts())

    expect(instruction.accounts).toHaveLength(8)
    expect(instruction.accounts!.at(-1)?.address).toBe(settleAccounts().system_program)
  })

  it('adds it when the shop has one', () => {
    const shopPolicy = address('9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM')
    const instruction = buildInstruction(
      'settle_payment',
      { amount: 1, memo: '' },
      { ...settleAccounts(), shop_policy: shopPolicy }
    )

    expect(instruction.accounts).toHaveLength(9)
    expect(instruction.accounts!.at(-1)).toEqual({
      address: shopPolicy,
      role: AccountRole.READONLY
    })
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
