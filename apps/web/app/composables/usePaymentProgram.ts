import {
  AccountRole,
  address,
  appendTransactionMessageInstruction,
  compileTransaction,
  createTransactionMessage,
  getAddressDecoder,
  getAddressEncoder,
  getProgramDerivedAddress,
  getTransactionEncoder,
  setTransactionMessageFeePayer,
  setTransactionMessageLifetimeUsingBlockhash,
  type AccountMeta,
  type Address,
  type Instruction
} from '@vue-solana/nuxt/kit'
import idl from '~/assets/payvand.idl.json'

const PAYMENT_SEED = new TextEncoder().encode('payment')

type IdlType = 'u64' | 'u8' | 'string' | 'pubkey'

interface IdlAccountDef {
  name: string
  writable?: boolean
  signer?: boolean
  address?: string
}

interface IdlArgDef {
  name: string
  type: IdlType
}

interface IdlInstruction {
  name: string
  discriminator: number[]
  accounts: IdlAccountDef[]
  args: IdlArgDef[]
}

interface ProgramIdl {
  address: string
  instructions: IdlInstruction[]
}

const programIdl = idl as unknown as ProgramIdl

/** Address of the deployed `payvand` program, taken from the IDL. */
export const PAYVAND_PROGRAM_ID = address(programIdl.address)

function findInstruction(name: string): IdlInstruction {
  const instruction = programIdl.instructions.find(ix => ix.name === name)
  if (!instruction) {
    throw new Error(`Unknown instruction "${name}" in the payvand IDL`)
  }
  return instruction
}

function encodeArgs(instruction: IdlInstruction, args: Record<string, unknown>): number[] {
  const bytes: number[] = []
  const push = (source: ArrayLike<number>) => {
    for (const byte of Array.from(source)) bytes.push(byte)
  }

  for (const arg of instruction.args) {
    const value = args[arg.name]
    if (value === undefined || value === null) {
      throw new Error(`Missing argument "${arg.name}" for ${instruction.name}`)
    }

    if (arg.type === 'u64') {
      const view = new DataView(new ArrayBuffer(8))
      view.setBigUint64(0, BigInt(value as number), true)
      push(new Uint8Array(view.buffer))
    } else if (arg.type === 'u8') {
      bytes.push(Number(value))
    } else if (arg.type === 'string') {
      const encoded = new TextEncoder().encode(String(value))
      const view = new DataView(new ArrayBuffer(4))
      view.setUint32(0, encoded.byteLength, true)
      push(new Uint8Array(view.buffer))
      push(encoded)
    } else {
      throw new Error(`Unsupported argument type "${String(arg.type)}"`)
    }
  }

  return bytes
}

/**
 * Builds a Kit `Instruction` straight from the Anchor IDL. Accounts the
 * program derives itself (PDAs) are passed in through `accounts`.
 */
export function buildInstruction(
  name: string,
  args: Record<string, unknown> = {},
  accounts: Record<string, Address> = {}
): Instruction {
  const instruction = findInstruction(name)

  const metas: AccountMeta[] = instruction.accounts.map((account) => {
    const resolved = accounts[account.name]
      ?? (account.address ? address(account.address) : undefined)
    if (!resolved) {
      throw new Error(`Missing account "${account.name}" for ${instruction.name}`)
    }

    let role = AccountRole.READONLY
    if (account.writable && account.signer) role = AccountRole.WRITABLE_SIGNER
    else if (account.signer) role = AccountRole.READONLY_SIGNER
    else if (account.writable) role = AccountRole.WRITABLE

    return { address: resolved, role }
  })

  return {
    programAddress: PAYVAND_PROGRAM_ID,
    accounts: metas,
    data: Uint8Array.from([...instruction.discriminator, ...encodeArgs(instruction, args)])
  }
}

/**
 * `seeds = [b"payment", seller, reference, amount_le]`, mirroring the Anchor
 * program. Baking the amount into the seeds is what makes a QR code safe to
 * hand out: any other amount derives a different address, so the seller's
 * screen — which watches this exact address — can never be shown as paid by a
 * smaller transfer.
 */
export async function derivePaymentPda(
  seller: Address,
  reference: Address,
  amount: number | bigint
): Promise<Address> {
  const encoder = getAddressEncoder()
  const amountBytes = new Uint8Array(8)
  new DataView(amountBytes.buffer).setBigUint64(0, BigInt(amount), true)
  const [pda] = await getProgramDerivedAddress({
    programAddress: PAYVAND_PROGRAM_ID,
    seeds: [PAYMENT_SEED, encoder.encode(seller), encoder.encode(reference), amountBytes]
  })
  return pda
}

/** Fresh 32 byte lookup key used as a PDA seed. */
export function createReference(): Address {
  return getAddressDecoder().decode(crypto.getRandomValues(new Uint8Array(32)))
}

/**
 * Compiles `instruction` into wire bytes, which is what
 * `useSolanaSignAndSendTransaction()` accepts.
 */
export async function compileWireTransaction(params: {
  instruction: Instruction
  payer: Address
  rpc: {
    getLatestBlockhash: () => {
      send: () => Promise<{
        value: Parameters<typeof setTransactionMessageLifetimeUsingBlockhash>[0]
      }>
    }
  }
}) {
  const { value: latestBlockhash } = await params.rpc.getLatestBlockhash().send()

  const message = setTransactionMessageLifetimeUsingBlockhash(
    latestBlockhash,
    setTransactionMessageFeePayer(
      params.payer,
      appendTransactionMessageInstruction(
        params.instruction,
        createTransactionMessage({ version: 0 })
      )
    )
  )

  return getTransactionEncoder().encode(compileTransaction(message))
}
