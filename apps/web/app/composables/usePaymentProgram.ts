import {
  AccountRole,
  address,
  appendTransactionMessageInstructions,
  compileTransaction,
  createTransactionMessage,
  getAddressDecoder,
  getAddressEncoder,
  getBase64Decoder,
  getBase64Encoder,
  getCompiledTransactionMessageDecoder,
  getProgramDerivedAddress,
  getTransactionDecoder,
  getTransactionEncoder,
  setTransactionMessageFeePayer,
  setTransactionMessageLifetimeUsingBlockhash,
  type AccountMeta,
  type Address,
  type Instruction
} from '@vue-solana/nuxt/kit'
import idl from '~/assets/payvand.idl.json'

const PAYMENT_SEED = new TextEncoder().encode('payment')
/** A settled invoice's permanent record, keyed by the invoice address. */
const PAYMENT_RECEIPT_SEED = new TextEncoder().encode('payment-receipt')
/** Token invoices live under their own prefix, with the mint as a seed. */
const TOKEN_PAYMENT_SEED = new TextEncoder().encode('token-payment')
/** The singleton fee policy every settlement reads. */
const CONFIG_SEED = new TextEncoder().encode('config')
/** The only account a fee may be sent to, and the receipt-rent refund source. */
const TREASURY_SEED = new TextEncoder().encode('treasury')

/** Legacy SPL token program — Circle's stablecoins use it. */
export const TOKEN_PROGRAM_ID = address('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA')
/** The program that derives token accounts from an owner and a mint. */
export const ASSOCIATED_TOKEN_PROGRAM_ID = address(
  'ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL'
)

/**
 * Circle's published mints. Stablecoins are worth one unit of their fiat, so
 * a price in them needs no exchange rate — only the mint to send it to.
 *
 * Adding a mint is two steps, not one: the program must also open its own
 * account for that mint (`open_treasury_token_account`, run by the deployer)
 * before any invoice in it can settle. The checkout reads that account and
 * refuses to let a buyer sign until it exists, so a mint added here without it
 * is unpurchasable rather than silently broken.
 */
const CURRENCY_MINTS: Record<'usdc' | 'eurc', { mainnet: string, devnet: string }> = {
  usdc: {
    mainnet: 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v',
    devnet: '4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU'
  },
  eurc: {
    // Circle lists one EURC address for both clusters.
    mainnet: 'HzwqbKZw8HxMN6bF2yFZNrht3c2iXXzpKcFu7uBEDKtr',
    devnet: 'HzwqbKZw8HxMN6bF2yFZNrht3c2iXXzpKcFu7uBEDKtr'
  }
}

/** The mint a stablecoin uses on a cluster, or null when it has none there. */
export function mintForCurrency(
  currency: 'usdc' | 'eurc',
  cluster: string
): Address | null {
  const entry = CURRENCY_MINTS[currency]
  const value = /^mainnet/.test(cluster) ? entry.mainnet : entry.devnet
  return value ? address(value) : null
}

/** The currencies a seller can offer on a cluster, in display order. */
export function currenciesForCluster(cluster: string): ('sol' | 'usdc' | 'eurc')[] {
  const offered: ('sol' | 'usdc' | 'eurc')[] = ['sol']
  if (mintForCurrency('usdc', cluster)) offered.push('usdc')
  if (mintForCurrency('eurc', cluster)) offered.push('eurc')
  return offered
}

/** The SPL Memo program — present on every Solana cluster. */
export const MEMO_PROGRAM_ID = address('MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr')

/** Where a wallet's own fee instructions live. */
export const COMPUTE_BUDGET_PROGRAM_ID = address(
  'ComputeBudget111111111111111111111111111111'
)

/**
 * `setComputeUnitLimit` — the one instruction that tells a wallet this app has
 * already chosen its compute budget.
 *
 * Phantom adds a compute-unit limit *and* a priority price of its own to any
 * transaction that carries neither, at the moment it signs. Both change the
 * message a relayer is about to co-sign, and the price is charged to the fee
 * payer — the relayer — which never gets it back. Naming the limit keeps the
 * sponsored transaction the one this app built, and 200,000 units is the limit
 * the runtime assumes anyway, so nothing that settles today runs short.
 */
export function computeUnitLimitInstruction(units = 200_000): Instruction {
  const data = new Uint8Array(5)
  data[0] = 2
  new DataView(data.buffer).setUint32(1, units, true)
  return { programAddress: COMPUTE_BUDGET_PROGRAM_ID, accounts: [], data }
}

type IdlType = 'u64' | 'u8' | 'string' | 'pubkey'

interface IdlAccountDef {
  name: string
  writable?: boolean
  signer?: boolean
  address?: string
  /** Anchor marks a trailing `Option<Account<..>>` here. */
  optional?: boolean
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

interface IdlError {
  code: number
  name: string
  msg?: string
}

interface ProgramIdl {
  address: string
  instructions: IdlInstruction[]
  errors?: IdlError[]
}

const programIdl = idl as unknown as ProgramIdl

/** Address of the deployed `payvand` program, taken from the IDL. */
export const PAYVAND_PROGRAM_ID = address(programIdl.address)

/** First 8 bytes of `sha256("account:<Name>")`, Anchor's account tag. */
export async function accountDiscriminator(name: string): Promise<Uint8Array> {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(`account:${name}`)
  )
  return new Uint8Array(digest.slice(0, 8))
}

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
 * The accounts the IDL cannot name, by the name the IDL gives them.
 *
 * `Interface<'info, TokenInterface>` lets the program accept the token program
 * at runtime, so Anchor has no address to write into the IDL: the account is
 * there with no `address`, and every token instruction — opening, paying from
 * or closing a token budget, settling a token invoice — fails to build without
 * one. The mints the app pays in are all classic SPL Token (the program rejects
 * Token-2022 by name), so the address is a constant rather than a caller's
 * choice.
 */
const IMPLIED_ACCOUNTS: Record<string, Address> = {
  token_program: TOKEN_PROGRAM_ID
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

  const metas: AccountMeta[] = []
  for (const account of instruction.accounts) {
    const resolved = accounts[account.name]
      ?? (account.address ? address(account.address) : undefined)
      ?? IMPLIED_ACCOUNTS[account.name]
    if (!resolved) {
      // Anchor's optional accounts are trailing, and the program reads the
      // shorter list as "none" — a settle with no trailing optional account is
      // the same transaction it always was. Anything missing that the IDL does
      // not mark optional is a caller mistake.
      if (account.optional) break
      throw new Error(`Missing account "${account.name}" for ${instruction.name}`)
    }

    let role = AccountRole.READONLY
    if (account.writable && account.signer) role = AccountRole.WRITABLE_SIGNER
    else if (account.signer) role = AccountRole.READONLY_SIGNER
    else if (account.writable) role = AccountRole.WRITABLE

    metas.push({ address: resolved, role })
  }

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

/**
 * `seeds = [b"config"]` — the fee policy account. Settlements pass it so the
 * program can read the fee rate, the mode, and the treasury bump; the client
 * names it rather than hardcoding a fee, so a policy change needs no release.
 */
export async function deriveConfigPda(): Promise<Address> {
  const [pda] = await getProgramDerivedAddress({
    programAddress: PAYVAND_PROGRAM_ID,
    seeds: [CONFIG_SEED]
  })
  return pda
}

/**
 * `seeds = [b"treasury"]` — where the fee goes. The program derives it too and
 * refuses any other account, so passing this one is what makes the fee
 * destination unredirectable.
 */
export async function deriveTreasuryPda(): Promise<Address> {
  const [pda] = await getProgramDerivedAddress({
    programAddress: PAYVAND_PROGRAM_ID,
    seeds: [TREASURY_SEED]
  })
  return pda
}

/**
 * The treasury's own token account for a mint: where a token invoice's fee
 * lands, so the buyer can hand over the price without carrying the protocol's
 * costs. It is opened once per mint by whoever runs the treasury, not per
 * payment.
 */
export async function deriveTreasuryTokenAccount(mint: Address): Promise<Address> {
  return deriveAssociatedTokenAddress(await deriveTreasuryPda(), mint)
}

/**
 * `seeds = [b"payment-receipt", invoice]`, mirroring the Anchor program.
 *
 * One receipt type covers SOL and token invoices, keyed by the invoice address:
 * once settlement closes the invoice account, the receipt is the only record
 * left, and this derivation works from the address in the QR code alone.
 */
export async function derivePaymentReceiptPda(invoice: Address): Promise<Address> {
  const encoder = getAddressEncoder()
  const [pda] = await getProgramDerivedAddress({
    programAddress: PAYVAND_PROGRAM_ID,
    seeds: [PAYMENT_RECEIPT_SEED, encoder.encode(invoice)]
  })
  return pda
}

/**
 * `seeds = [b"token-payment", seller, reference, mint, amount_le]`. The mint is
 * in the seeds, so a USDC price and a SOL price can never settle each other
 * even when they share a reference and an amount.
 */
export async function deriveTokenPaymentPda(
  seller: Address,
  reference: Address,
  mint: Address,
  amount: number | bigint
): Promise<Address> {
  const encoder = getAddressEncoder()
  const amountBytes = new Uint8Array(8)
  new DataView(amountBytes.buffer).setBigUint64(0, BigInt(amount), true)
  const [pda] = await getProgramDerivedAddress({
    programAddress: PAYVAND_PROGRAM_ID,
    seeds: [
      TOKEN_PAYMENT_SEED,
      encoder.encode(seller),
      encoder.encode(reference),
      encoder.encode(mint),
      amountBytes
    ]
  })
  return pda
}

/**
 * The address of `owner`'s token account for `mint`. The program creates it
 * when it does not exist yet, so this is only ever needed to name it.
 */
export async function deriveAssociatedTokenAddress(
  owner: Address,
  mint: Address
): Promise<Address> {
  const encoder = getAddressEncoder()
  const [pda] = await getProgramDerivedAddress({
    programAddress: ASSOCIATED_TOKEN_PROGRAM_ID,
    seeds: [encoder.encode(owner), encoder.encode(TOKEN_PROGRAM_ID), encoder.encode(mint)]
  })
  return pda
}

/** Fresh 32 byte lookup key used as a PDA seed. */
export function createReference(): Address {
  return getAddressDecoder().decode(crypto.getRandomValues(new Uint8Array(32)))
}

/**
 * A Memo-program instruction carrying a human readable description. Wallets
 * render it in the approval screen and on explorers, so an Anchor
 * instruction is never just an opaque blob of hex.
 */
export function buildMemoInstruction(memo: string, signer: Address): Instruction {
  return {
    programAddress: MEMO_PROGRAM_ID,
    accounts: [{ address: signer, role: AccountRole.READONLY_SIGNER }],
    data: new TextEncoder().encode(memo)
  }
}

/**
 * Plain-language text for an Anchor custom error code from the IDL, or null
 * when the code is not one of the program's own errors.
 */
export function describeProgramErrorCode(code: number): string | null {
  const known = programIdl.errors?.find(error => error.code === code)
  return known?.msg ?? known?.name ?? null
}

export interface WireTransactionParams {
  instruction: Instruction | Instruction[]
  /** The account that pays the network fee: the buyer, or its sponsor. */
  payer: Address
  rpc: {
    getLatestBlockhash: () => {
      send: () => Promise<{
        value: Parameters<typeof setTransactionMessageLifetimeUsingBlockhash>[0]
      }>
    }
  }
}

/**
 * Compiles `instruction` (or a list of them, e.g. the instruction plus a
 * memo) into the version 0 transaction a wallet signs, before any signature is
 * on it. `payer` is its fee payer, which is also the address the wallet's
 * signature joins — a sponsored settle names the sponsor there instead.
 */
export async function compileTransactionForSigning(params: WireTransactionParams) {
  const { value: latestBlockhash } = await params.rpc.getLatestBlockhash().send()

  const instructions = Array.isArray(params.instruction)
    ? params.instruction
    : [params.instruction]

  const message = setTransactionMessageLifetimeUsingBlockhash(
    latestBlockhash,
    setTransactionMessageFeePayer(
      params.payer,
      appendTransactionMessageInstructions(instructions, createTransactionMessage({ version: 0 }))
    )
  )

  return compileTransaction(message)
}

/**
 * The same transaction as wire bytes, which is what
 * `useSolanaSignAndSendTransaction()` accepts.
 */
export async function compileWireTransaction(params: WireTransactionParams) {
  return getTransactionEncoder().encode(await compileTransactionForSigning(params))
}

/** The bytes `getTransactionEncoder()` produces: a transaction as it goes on the wire. */
export type WireBytes = ReturnType<ReturnType<typeof getTransactionEncoder>['encode']>

/**
 * Wire bytes as base64: what `POST /api/sponsor/settle` takes back once the
 * wallet has signed the relayer's prepared transaction. The copy into a
 * plain `Uint8Array` is only to satisfy the decoder's buffer generic — the bytes
 * are the ones the wallet handed back, unchanged.
 */
export function base64WireTransaction(wire: ArrayLike<number>): string {
  return getBase64Decoder().decode(new Uint8Array(wire))
}

/** Base64 wire bytes from the API, as the bytes a wallet signs. */
export function wireFromBase64(transaction: string): WireBytes {
  return getBase64Encoder().encode(transaction) as WireBytes
}

function sameBytes(a: ArrayLike<number> | undefined, b: ArrayLike<number> | undefined): boolean {
  const left = a ?? []
  const right = b ?? []
  if (left.length !== right.length) return false
  for (let index = 0; index < left.length; index++) {
    if (left[index] !== right[index]) return false
  }
  return true
}

/**
 * Whether a transaction the relayer prepared is exactly this page's settle,
 * checked before the buyer's wallet is asked to sign it.
 *
 * The relayer builds the settle itself and signs first, which is what lets the
 * wallet preview it (`docs/payment-costs.md`) — but then the bytes the buyer is
 * asked to sign come from a server. This is the buyer's own check on them: the
 * relayer is the fee payer, nothing is looked up from a table, and the message
 * holds two instructions — a compute-unit limit and `settle` — where `settle`
 * is this page's instruction, the same program, data and accounts in order.
 * Anything else (another invoice, an extra transfer, a priority price) is
 * refused without a wallet prompt.
 */
export function preparedSettleMatches(
  wire: ArrayLike<number>,
  relayer: Address,
  settle: Instruction
): boolean {
  try {
    const transaction = getTransactionDecoder().decode(new Uint8Array(wire))
    const message = getCompiledTransactionMessageDecoder().decode(transaction.messageBytes)
    // The relayer builds version 0 and nothing else.
    if (message.version !== 0) return false
    const keys = message.staticAccounts
    if (keys[0] !== relayer) return false
    if ((message.addressTableLookups?.length ?? 0) > 0) return false
    if (message.instructions.length !== 2) return false
    const [limit, settled] = message.instructions
    const expectedLimit = computeUnitLimitInstruction()
    if (keys[limit!.programAddressIndex] !== expectedLimit.programAddress) return false
    if ((limit!.accountIndices?.length ?? 0) !== 0) return false
    if (!sameBytes(limit!.data, expectedLimit.data)) return false
    if (keys[settled!.programAddressIndex] !== settle.programAddress) return false
    if (!sameBytes(settled!.data, settle.data)) return false
    const named = (settled!.accountIndices ?? []).map(index => keys[index])
    const wanted = (settle.accounts ?? []).map(account => account.address)
    return named.length === wanted.length && named.every((key, index) => key === wanted[index])
  } catch {
    return false
  }
}
