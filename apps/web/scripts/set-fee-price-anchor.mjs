#!/usr/bin/env node
// Writes the SOL price a permissionless `refresh_fee_price` is measured against.
//
// The refresh is permissionless, so its band cannot be measured from the flat it
// is about to rewrite: a caller could move that one band's worth every cooldown
// until the record fee no longer matched the rent the treasury refunds. The band
// is instead measured from this anchor — `["fee-anchor"]`, an account only the
// config's admin can write. `set_fee_price_anchor` creates it on the first call,
// prices both token flats from the cluster's own receipt rent at that price, and
// starts the refresh cooldown; it is also the deliberate re-peg when the price
// should move further than the band in one step.
//
//   pnpm fee:anchor --cluster devnet
//   node scripts/set-fee-price-anchor.mjs --cluster mainnet-beta --keypair ~/.config/solana/id.json
//   node scripts/set-fee-price-anchor.mjs --sol-eur 61.99 --sol-usd 67.20   # no rate feed
//
// The keypair must be the config's admin (the upgrade authority that initialized
// it), and the anchor must be written before any refresh can run.
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

const INSTRUCTION = 'set_fee_price_anchor'
const SYSTEM_PROGRAM_ID = address('11111111111111111111111111111111')
const RATE_FEED = 'https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=eur,usd'

function fail(message) {
  console.error(`error: ${message}`)
  process.exit(1)
}

function parseArgs(argv) {
  const args = { cluster: 'devnet', keypair: join(homedir(), '.config/solana/id.json') }
  for (let i = 0; i < argv.length; i += 2) {
    const flag = argv[i]
    const value = argv[i + 1]
    if (!value || !flag.startsWith('--')) fail(`unexpected argument ${flag}; use --cluster, --keypair, --rpc, --sol-eur, --sol-usd`)
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

/** Whole micro-units of a price: €61.99 is 61,990,000 micros. */
function micros(price, name) {
  const value = Number(price)
  if (!Number.isFinite(value) || value <= 0) fail(`${name} is not a price: ${price}`)
  return BigInt(Math.round(value * 1_000_000))
}

async function prices(args) {
  if (args['sol-eur'] && args['sol-usd']) {
    return { eur: micros(args['sol-eur'], '--sol-eur'), usd: micros(args['sol-usd'], '--sol-usd'), source: 'flags' }
  }
  const response = await fetch(RATE_FEED, { headers: { accept: 'application/json' } })
  if (!response.ok) fail(`the rate feed answered ${response.status}; pass --sol-eur and --sol-usd instead`)
  const body = await response.json()
  const eur = body?.solana?.eur
  const usd = body?.solana?.usd
  if (!eur || !usd) fail('the rate feed did not have a SOL price')
  return { eur: micros(eur, 'SOL in EUR'), usd: micros(usd, 'SOL in USD'), source: RATE_FEED }
}

async function main() {
  const args = parseArgs(process.argv.slice(2))
  const rpc = createSolanaRpc(args.rpc ?? endpointFor(args.cluster))

  const idlPath = fileURLToPath(new URL('../app/assets/payvand.idl.json', import.meta.url))
  const idl = JSON.parse(await readFile(idlPath, 'utf8'))
  const program = address(idl.address)
  const definition = idl.instructions.find(instruction => instruction.name === INSTRUCTION)
  if (!definition) fail(`${idlPath} has no ${INSTRUCTION} instruction — run \`pnpm anchor:build\` after deploying the program`)

  const secretKey = Uint8Array.from(JSON.parse(await readFile(args.keypair, 'utf8')))
  const admin = await createKeyPairSignerFromBytes(secretKey)
  const [config] = await getProgramDerivedAddress({ programAddress: program, seeds: [new TextEncoder().encode('config')] })
  const [anchor] = await getProgramDerivedAddress({ programAddress: program, seeds: [new TextEncoder().encode('fee-anchor')] })

  const configInfo = (await rpc.getAccountInfo(config, { encoding: 'base64' }).send()).value
  if (!configInfo) fail(`no config account at ${config}: initialize it before anchoring a price`)

  const { eur, usd, source } = await prices(args)
  const known = { admin: admin.address, config, anchor, system_program: SYSTEM_PROGRAM_ID }
  const accounts = definition.accounts.map(({ name, writable, signer }) => {
    const resolved = known[name]
    if (!resolved) fail(`the IDL asks for an account this script cannot place: ${name}`)
    return { address: resolved, role: (signer === true ? 2 : 0) + (writable === true ? 1 : 0) }
  })

  // Two `u64` arguments, little-endian, in the order the instruction declares.
  const data = new Uint8Array(definition.discriminator.length + 16)
  data.set(definition.discriminator, 0)
  const argsView = new DataView(data.buffer, definition.discriminator.length, 16)
  argsView.setBigUint64(0, usd, true)
  argsView.setBigUint64(8, eur, true)

  console.log(`cluster  ${args.cluster}`)
  console.log(`admin    ${admin.address}`)
  console.log(`anchor   ${anchor}`)
  console.log(`price    $${Number(usd) / 1e6} and €${Number(eur) / 1e6} a SOL (${source})`)

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
      console.log('anchored — the flats are priced at this rate; `pnpm fee:refresh` may now move them within 10% of it')
      return
    }
    await new Promise(resolve => setTimeout(resolve, 1_200))
  }
  fail(`no confirmation after a minute: check ${signature}`)
}

await main()
