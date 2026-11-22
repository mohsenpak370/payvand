#!/usr/bin/env node
// Upgrades the on-chain fee config to the current layout.
//
// `migrate_config` takes the `FeePolicy` it should install and grows the account
// to `CONFIG_SPACE`, carrying the admin, fee mode, session float and bumps over.
// This sends it with the policy the account already holds — the bytes at the
// `FeePolicy` offset — so the rates, tiers and mints survive the layout change.
// The program zeroes both token flats on the way in and leaves `flats_updated_at`
// at zero: flats priced for an older rent and SOL rate would be too far from the
// live one for the refresh band, so the first refresh after a migration is meant
// to start from zero and re-peg freely. Run `pnpm fee:refresh` immediately after
// — until then a token settlement is charged the percentage alone, with no
// record floor.
//
//   pnpm config:migrate --cluster devnet
//
// Only the recorded admin may send this, so the keypair must be the upgrade
// authority that initialized the config.
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

const INSTRUCTION = 'migrate_config'
const SYSTEM_PROGRAM_ID = address('11111111111111111111111111111111')
/** Where `FeePolicy` starts in `Config`: 8 of discriminator, 32 of admin. */
const POLICY_OFFSET = 40
const POLICY_LEN = 2 + 2 + 8 + 8 + 8 + 8 + 32 + 32
/** The sizes the program has written: version 1, version 2, version 3. */
const KNOWN_SIZES = [62, 152, 160]

function fail(message) {
  console.error(`error: ${message}`)
  process.exit(1)
}

function parseArgs(argv) {
  const args = { cluster: 'devnet', keypair: join(homedir(), '.config/solana/id.json') }
  for (let i = 0; i < argv.length; i += 2) {
    const flag = argv[i]
    const value = argv[i + 1]
    if (!value || !flag.startsWith('--')) fail(`unexpected argument ${flag}; use --cluster, --keypair, --rpc`)
    args[flag.slice(2)] = value
  }
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
  if (!KNOWN_SIZES.includes(bytes.length)) fail(`config is ${bytes.length} bytes, which is not a layout this program wrote`)
  if (bytes.length === 160) {
    console.log(`config ${config} is already the current layout (${bytes.length} bytes) — nothing to migrate`)
    return
  }

  const policy = bytes.slice(POLICY_OFFSET, POLICY_OFFSET + POLICY_LEN)
  const view = new DataView(policy.buffer, policy.byteOffset, policy.byteLength)
  console.log(`cluster   ${args.cluster}`)
  console.log(`admin     ${admin.address}`)
  console.log(`config    ${config} (${bytes.length} bytes)`)
  console.log(`policy    ${view.getUint16(0, true)} / ${view.getUint16(2, true)} bps, tiers ${view.getBigUint64(4, true)} / ${view.getBigUint64(12, true)}, flats ${view.getBigUint64(20, true)} / ${view.getBigUint64(28, true)}`)

  const known = { admin: admin.address, config, system_program: SYSTEM_PROGRAM_ID }
  const accounts = definition.accounts.map(({ name, writable, signer }) => {
    const resolved = known[name]
    if (!resolved) fail(`the IDL asks for an account this script cannot place: ${name}`)
    return { address: resolved, role: (signer === true ? 2 : 0) + (writable === true ? 1 : 0) }
  })

  const data = new Uint8Array(definition.discriminator.length + policy.length)
  data.set(definition.discriminator, 0)
  data.set(policy, definition.discriminator.length)

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
      console.log(`signature ${signature}`)
      console.log('migrated — run `pnpm fee:refresh` next so the token flats are priced from today\'s SOL')
      return
    }
    await new Promise(resolve => setTimeout(resolve, 1_200))
  }
  fail(`no confirmation after a minute: check ${signature}`)
}

await main()
