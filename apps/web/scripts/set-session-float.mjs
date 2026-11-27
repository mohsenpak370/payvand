#!/usr/bin/env node
// Sets `Config::session_fee_float`, the lamports parked on a session key so its
// silent payments can front their own rents and signature.
//
// `update_config` takes the whole policy, so this resends the bytes the account
// already holds — the rates, tiers, flats, mints and network fees are untouched
// — and changes only the float. A config cannot go below the program's
// `MIN_SESSION_FEE_FLOAT` (3,716,640): one SOL payment's invoice and receipt
// rents, which a key at the floor fronts and the pot refunds in the same
// transaction.
//
//   pnpm config:float --float 3716640 --cluster devnet
//   node scripts/set-session-float.mjs --float 3000000          # refused on chain
//
// Only the config's admin may send this, so the keypair must be the upgrade
// authority that initialized it. The write also stamps `flats_updated_at`, which
// starts `refresh_fee_price`'s cooldown; the flats themselves are unchanged.
import { readFile } from 'node:fs/promises'
import { homedir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import {
  address,
  appendTransactionMessageInstructions,
  createKeyPairSignerFromBytes,
  createSolanaRpc,
  createTransactionMessage,
  getBase64EncodedWireTransaction,
  getProgramDerivedAddress,
  pipe,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners
} from '@vue-solana/nuxt/kit'

const INSTRUCTION = 'update_config'
const SYSTEM_PROGRAM_ID = address('11111111111111111111111111111111')
/** Where `FeePolicy` starts in `Config` and how long it is in the v4 layout. */
const POLICY_OFFSET = 40
const POLICY_LEN = 2 + 2 + 8 + 8 + 8 + 8 + 32 + 32 + 8 + 8 + 8
/** `fee_mode` follows the policy; the float follows that. */
const FEE_MODE_OFFSET = POLICY_OFFSET + POLICY_LEN
const FLOAT_OFFSET = FEE_MODE_OFFSET + 1
/** What the program accepts: `MIN_SESSION_FEE_FLOAT`. */
const MIN_FLOAT = 3_716_640n

function fail(message) {
  console.error(`error: ${message}`)
  process.exit(1)
}

function parseArgs(argv) {
  const args = { cluster: 'devnet', keypair: join(homedir(), '.config/solana/id.json') }
  for (let i = 0; i < argv.length; i += 2) {
    const flag = argv[i]
    const value = argv[i + 1]
    if (!value || !flag.startsWith('--')) fail(`unexpected argument ${flag}; use --float, --cluster, --keypair, --rpc`)
    args[flag.slice(2)] = value
  }
  if (!args.float || !/^\d+$/.test(args.float)) fail('--float is the lamport value to set, e.g. --float 3716640')
  return args
}

function endpointFor(cluster) {
  if (cluster === 'mainnet-beta' || cluster === 'mainnet') return 'https://api.mainnet-beta.solana.com'
  if (cluster === 'devnet') return 'https://api.devnet.solana.com'
  if (cluster === 'testnet') return 'https://api.testnet.solana.com'
  return `https://api.${cluster}.solana.com`
}

async function main() {
  const args = parseArgs(process.argv.slice(2))
  const wanted = BigInt(args.float)
  if (wanted < MIN_FLOAT) fail(`${wanted} is below MIN_SESSION_FEE_FLOAT (${MIN_FLOAT}); the program would refuse it`)
  const rpc = createSolanaRpc(args.rpc ?? endpointFor(args.cluster))

  const idlPath = fileURLToPath(new URL('../app/assets/payvand.idl.json', import.meta.url))
  const idl = JSON.parse(await readFile(idlPath, 'utf8'))
  const program = address(idl.address)
  const definition = idl.instructions.find(instruction => instruction.name === INSTRUCTION)
  if (!definition) fail(`${idlPath} has no ${INSTRUCTION} instruction — run \`pnpm anchor:build\``)

  const secretKey = Uint8Array.from(JSON.parse(await readFile(args.keypair, 'utf8')))
  const admin = await createKeyPairSignerFromBytes(secretKey)
  const [config] = await getProgramDerivedAddress({ programAddress: program, seeds: [new TextEncoder().encode('config')] })

  const info = (await rpc.getAccountInfo(config, { encoding: 'base64' }).send()).value
  if (!info) fail(`no config account at ${config}: initialize it first`)
  const bytes = Uint8Array.from(atob(info.data[0]), char => char.charCodeAt(0))
  if (bytes.length < FLOAT_OFFSET + 8) fail(`config is ${bytes.length} bytes, too short for a session float`)

  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  const current = view.getBigUint64(FLOAT_OFFSET, true)
  console.log(`cluster   ${args.cluster}`)
  console.log(`admin     ${admin.address}`)
  console.log(`config    ${config}`)
  console.log(`policy    ${view.getUint16(POLICY_OFFSET, true)} / ${view.getUint16(POLICY_OFFSET + 2, true)} bps (unchanged)`)
  console.log(`float     ${current} → ${wanted} lamports`)
  if (current === wanted) {
    console.log('already set — nothing to send')
    return
  }

  // The policy and fee mode go back exactly as they are; only the float moves.
  const data = new Uint8Array(definition.discriminator.length + POLICY_LEN + 1 + 8)
  data.set(definition.discriminator, 0)
  data.set(bytes.slice(POLICY_OFFSET, FEE_MODE_OFFSET), definition.discriminator.length)
  data[definition.discriminator.length + POLICY_LEN] = bytes[FEE_MODE_OFFSET]
  new DataView(data.buffer).setBigUint64(data.length - 8, wanted, true)

  const known = { admin: admin.address, config, system_program: SYSTEM_PROGRAM_ID }
  const accounts = definition.accounts.map(({ name, writable, signer }) => {
    const resolved = known[name]
    if (!resolved) fail(`the IDL asks for an account this script cannot place: ${name}`)
    return { address: resolved, role: (signer === true ? 2 : 0) + (writable === true ? 1 : 0) }
  })

  const { value: latest } = await rpc.getLatestBlockhash().send()
  const message = pipe(
    createTransactionMessage({ version: 0 }),
    m => setTransactionMessageFeePayerSigner(admin, m),
    m => setTransactionMessageLifetimeUsingBlockhash(latest, m),
    m => appendTransactionMessageInstructions([{ programAddress: program, accounts, data }], m)
  )
  const signature = await rpc
    .sendTransaction(getBase64EncodedWireTransaction(await signTransactionMessageWithSigners(message)), { encoding: 'base64' })
    .send()

  for (let attempt = 0; attempt < 40; attempt++) {
    const { value } = await rpc.getSignatureStatuses([signature]).send()
    const status = value[0]
    if (status?.err) fail(`the transaction failed: ${JSON.stringify(status.err)} (${signature})`)
    if (status && (status.confirmationStatus === 'confirmed' || status.confirmationStatus === 'finalized')) {
      const after = Uint8Array.from(atob((await rpc.getAccountInfo(config, { encoding: 'base64' }).send()).value.data[0]), c => c.charCodeAt(0))
      const landed = new DataView(after.buffer, after.byteOffset, after.byteLength).getBigUint64(FLOAT_OFFSET, true)
      console.log(`signature ${signature}`)
      console.log(`config now holds float ${landed}`)
      if (landed !== wanted) fail(`the write landed as ${landed}, expected ${wanted}`)
      return
    }
    await new Promise(resolve => setTimeout(resolve, 1_200))
  }
  fail(`no confirmation after a minute: check ${signature}`)
}

await main()
