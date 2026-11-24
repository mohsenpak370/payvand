#!/usr/bin/env node
// One real SOL settle on devnet, twice: once with a relayer as fee payer and
// sponsor, once with the payer sponsoring itself. It checks what the fee model
// promises — the payer's balance falls by exactly the price, the sponsor ends
// whole (it fronts both record rents and two signatures, and the price repays
// both), and the seller receives the price less the rents and the policy's
// network fee times the signatures the path carried.
//
//   node scripts/devnet-fee-model-check.mjs --cluster devnet
//   node scripts/devnet-fee-model-check.mjs --relayer .secrets/sponsor.json
//
// The policy is read from the on-chain `Config` (v4), so the fee it asserts is
// the fee the deployed program charges. Devnet only by default.
import { execFileSync } from 'node:child_process'
import { readFile, mkdtemp, rm } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import {
  addSignersToTransactionMessage,
  address,
  appendTransactionMessageInstructions,
  createKeyPairSignerFromBytes,
  createSolanaRpc,
  createTransactionMessage,
  getAddressEncoder,
  getBase64EncodedWireTransaction,
  getProgramDerivedAddress,
  pipe,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners
} from '@vue-solana/nuxt/kit'

const SYSTEM_PROGRAM_ID = address('11111111111111111111111111111111')
/** `PAYMENT_RECEIPT_SPACE`: 8 of discriminator, 32 of payment, 8 of settled_at. */
const PAYMENT_RECEIPT_SPACE = 48
const encoder = new TextEncoder()
const bytes = getAddressEncoder()
const LAMPORTS_PER_SOL = 1_000_000_000
/** What the bank charges for one signature. */
const SIGNATURE_FEE = 5_000

function fail(message) {
  console.error(`error: ${message}`)
  process.exit(1)
}

function parseArgs(argv) {
  const args = { cluster: 'devnet', keypair: join(homedir(), '.config/solana/id.json'), relayer: '.secrets/sponsor.json' }
  for (let i = 0; i < argv.length; i += 2) {
    const flag = argv[i]
    const value = argv[i + 1]
    if (!value || !flag.startsWith('--')) fail(`unexpected argument ${flag}; use --cluster, --keypair, --relayer, --rpc`)
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

function u64(value) {
  const out = new Uint8Array(8)
  new DataView(out.buffer).setBigUint64(0, BigInt(value), true)
  return out
}

function u32LengthPrefixed(text) {
  const body = encoder.encode(text)
  const out = new Uint8Array(4 + body.length)
  new DataView(out.buffer).setUint32(0, body.length, true)
  out.set(body, 4)
  return out
}

function concat(...parts) {
  const length = parts.reduce((total, part) => total + part.length, 0)
  const out = new Uint8Array(length)
  let at = 0
  for (const part of parts) {
    out.set(part, at)
    at += part.length
  }
  return out
}

function programInstruction(idl, name, placements, args = new Uint8Array()) {
  const definition = idl.instructions.find(instruction => instruction.name === name)
  if (!definition) fail(`the IDL has no ${name} instruction`)
  const accounts = definition.accounts.flatMap(({ name: accountName, writable, signer, optional }) => {
    const resolved = placements[accountName]
    // A trailing optional account the caller does not have is left out, which
    // is what the program reads as "no shop policy for this invoice".
    if (!resolved) {
      if (optional === true) return []
      fail(`${name} asks for an account this script cannot place: ${accountName}`)
    }
    return [{ address: resolved, role: (signer === true ? 2 : 0) + (writable === true ? 1 : 0) }]
  })
  return {
    programAddress: address(idl.address),
    accounts,
    data: concat(Uint8Array.from(definition.discriminator), args)
  }
}

function describe(rejection) {
  if (!rejection) return 'no reason given'
  if (rejection instanceof Error) {
    const logs = rejection.context?.logs
    return `${rejection.message}${Array.isArray(logs) ? `\n  ${logs.join('\n  ')}` : ''}`
  }
  return JSON.stringify(rejection, (_key, value) => (typeof value === 'bigint' ? value.toString() : value))
}

async function main() {
  const args = parseArgs(process.argv.slice(2))
  const endpoint = args.rpc ?? endpointFor(args.cluster)
  const rpc = createSolanaRpc(endpoint)

  const idlPath = fileURLToPath(new URL('../app/assets/payvand.idl.json', import.meta.url))
  const idl = JSON.parse(await readFile(idlPath, 'utf8'))
  const program = address(idl.address)

  const admin = await createKeyPairSignerFromBytes(Uint8Array.from(JSON.parse(await readFile(args.keypair, 'utf8'))))
  const relayer = await createKeyPairSignerFromBytes(Uint8Array.from(JSON.parse(await readFile(args.relayer, 'utf8'))))

  async function pda(seeds) {
    const [found] = await getProgramDerivedAddress({ programAddress: program, seeds })
    return found
  }

  const config = await pda([encoder.encode('config')])
  const treasury = await pda([encoder.encode('treasury')])

  const info = (await rpc.getAccountInfo(config, { encoding: 'base64' }).send()).value
  if (!info) fail(`no config account at ${config}`)
  const raw = Uint8Array.from(atob(info.data[0]), char => char.charCodeAt(0))
  if (raw.length !== 184) fail(`config is ${raw.length} bytes: this check wants the v4 layout (184)`)
  const view = new DataView(raw.buffer, raw.byteOffset, raw.byteLength)
  const policy = {
    feeBpsBelow: view.getUint16(40, true),
    feeBpsAbove: view.getUint16(42, true),
    solTier: Number(view.getBigUint64(44, true)),
    flatUsdc: Number(view.getBigUint64(60, true)),
    flatEurc: Number(view.getBigUint64(68, true)),
    networkFee: Number(view.getBigUint64(140, true)),
    networkFeeUsdc: Number(view.getBigUint64(148, true)),
    networkFeeEurc: Number(view.getBigUint64(156, true))
  }

  async function lamports(account) {
    return Number((await rpc.getBalance(account).send()).value)
  }

  async function freshSigner(dir, name) {
    const path = join(dir, `${name}.json`)
    execFileSync('solana-keygen', ['new', '--no-bip39-passphrase', '--silent', '-o', path], { encoding: 'utf8' })
    return createKeyPairSignerFromBytes(Uint8Array.from(JSON.parse(await readFile(path, 'utf8'))))
  }

  async function send(feePayer, instructions) {
    const { value: lifetime } = await rpc.getLatestBlockhash().send()
    let message = pipe(
      createTransactionMessage({ version: 0 }),
      m => setTransactionMessageFeePayerSigner(feePayer, m),
      m => setTransactionMessageLifetimeUsingBlockhash(lifetime, m),
      m => appendTransactionMessageInstructions(instructions, m)
    )
    message = addSignersToTransactionMessage([feePayer], message)
    const signature = await rpc
      .sendTransaction(getBase64EncodedWireTransaction(await signTransactionMessageWithSigners(message)), { encoding: 'base64' })
      .send()
    for (let attempt = 0; attempt < 90; attempt++) {
      const status = (await rpc.getSignatureStatuses([signature]).send()).value[0]
      if (status?.err) fail(`transaction ${signature} failed: ${describe(status.err)}`)
      if (status && (status.confirmationStatus === 'confirmed' || status.confirmationStatus === 'finalized')) return signature
      await new Promise(resolve => setTimeout(resolve, 1_200))
    }
    fail(`no confirmation for ${signature}`)
  }

  /**
   * `fee_for` on the SOL path: the boundary rate, less the receipt's rent the
   * seller hands over directly, floored at this path's signatures. The floor is
   * what the treasury recharges, so the treasury keeps `fee - network` — which is
   * zero when the rate cannot cover the record, and that is the case a small
   * invoice lands in.
   */
  const receiptRent = Number(await rpc.getMinimumBalanceForRentExemption(BigInt(PAYMENT_RECEIPT_SPACE)).send())
  function rateFor(amount) {
    const below = Math.min(amount, policy.solTier)
    return Math.floor((below * policy.feeBpsBelow + (amount - below) * policy.feeBpsAbove) / 10_000)
  }
  function feeFor(amount, signatures) {
    return Math.max(0, rateFor(amount) - receiptRent, policy.networkFee * signatures)
  }

  const dir = await mkdtemp(join(tmpdir(), 'payvand-fees-'))
  try {
    console.log(`cluster   ${args.cluster} (${endpoint})`)
    console.log(`program   ${program}`)
    console.log(`config    ${config} (184 bytes, v4)`)
    console.log(`policy    ${policy.feeBpsBelow}/${policy.feeBpsAbove} bps, network ${policy.networkFee} lamports per signature`)
    console.log(`relayer   ${relayer.address} (${await lamports(relayer.address)} lamports)`)

    const sponsoredSeller = await freshSigner(dir, 'seller-sponsored')
    const sponsoredBuyer = await freshSigner(dir, 'buyer-sponsored')
    const ownSeller = await freshSigner(dir, 'seller-own')
    const ownBuyer = await freshSigner(dir, 'buyer-own')

    // The seller needs no lamports at all; the buyers hold only what they spend.
    const amount = 50_000_000
    for (const buyer of [sponsoredBuyer, ownBuyer, sponsoredSeller, ownSeller]) {
      await send(admin, [{
        programAddress: SYSTEM_PROGRAM_ID,
        accounts: [
          { address: admin.address, role: 3 },
          { address: buyer.address, role: 1 }
        ],
        data: concat(new Uint8Array([2, 0, 0, 0]), u64(LAMPORTS_PER_SOL / 5))
      }])
    }

    const results = []

    async function settle({ name, buyer, seller, sponsor, signatures }) {
      const reference = (await freshSigner(dir, `ref-${name}`)).address
      const payment = await pda([
        encoder.encode('payment'),
        bytes.encode(seller.address),
        bytes.encode(reference),
        u64(amount)
      ])
      const receipt = await pda([encoder.encode('payment-receipt'), bytes.encode(payment)])

      const before = {
        buyer: await lamports(buyer.address),
        seller: await lamports(seller.address),
        sponsor: await lamports(sponsor.address),
        treasury: await lamports(treasury)
      }

      const instruction = programInstruction(idl, 'settle_payment', {
        payer: buyer.address,
        payment,
        seller: seller.address,
        receipt,
        reference,
        config,
        treasury,
        system_program: SYSTEM_PROGRAM_ID,
        sponsor: sponsor.address
      }, concat(u64(amount), u32LengthPrefixed(`fee-model ${name}`)))

      // Both the buyer and the sponsor sign: the buyer signs in its own slot,
      // exactly as the checkout has the wallet sign a relayer's message.
      const { value: lifetime } = await rpc.getLatestBlockhash().send()
      const message = addSignersToTransactionMessage(
        [buyer, sponsor],
        pipe(
          createTransactionMessage({ version: 0 }),
          m => setTransactionMessageFeePayerSigner(sponsor, m),
          m => setTransactionMessageLifetimeUsingBlockhash(lifetime, m),
          m => appendTransactionMessageInstructions([instruction], m)
        )
      )
      const signature = await rpc
        .sendTransaction(getBase64EncodedWireTransaction(await signTransactionMessageWithSigners(message)), { encoding: 'base64' })
        .send()
      let confirmed = false
      for (let attempt = 0; attempt < 90 && !confirmed; attempt++) {
        const status = (await rpc.getSignatureStatuses([signature]).send()).value[0]
        if (status?.err) fail(`${name} failed: ${describe(status.err)}`)
        confirmed = Boolean(status && (status.confirmationStatus === 'confirmed' || status.confirmationStatus === 'finalized'))
        if (!confirmed) await new Promise(resolve => setTimeout(resolve, 1_200))
      }
      if (!confirmed) fail(`${name} did not confirm: ${signature}`)

      const after = {
        buyer: await lamports(buyer.address),
        seller: await lamports(seller.address),
        sponsor: await lamports(sponsor.address),
        treasury: await lamports(treasury)
      }
      const fee = feeFor(amount, signatures)
      results.push({ name, signature, before, after, fee, signatures })
      console.log(`\n${name}`)
      console.log(`  signature      ${signature}`)
      console.log(`  payer          ${before.buyer} → ${after.buyer} (${after.buyer - before.buyer})`)
      console.log(`  seller         ${before.seller} → ${after.seller} (${after.seller - before.seller})`)
      console.log(`  sponsor        ${before.sponsor} → ${after.sponsor} (${after.sponsor - before.sponsor})`)
      console.log(`  treasury       ${before.treasury} → ${after.treasury} (${after.treasury - before.treasury})`)
      console.log(`  policy fee     ${fee} for ${signatures} signature(s) (rate ${rateFor(amount)}, receipt rent ${receiptRent})`)
      return { before, after, fee, signatures }
    }

    const sponsored = await settle({ name: 'sponsored SOL', buyer: sponsoredBuyer, seller: sponsoredSeller, sponsor: relayer, signatures: 2 })
    const own = await settle({ name: 'self-sponsored SOL', buyer: ownBuyer, seller: ownSeller, sponsor: ownBuyer, signatures: 1 })

    const checks = []
    checks.push(['sponsored payer paid exactly the price', sponsored.after.buyer - sponsored.before.buyer === -amount])
    checks.push(['relayer only bought back what it fronted', sponsored.after.sponsor - sponsored.before.sponsor === 0])
    checks.push([
      'sponsored treasury kept fee - network',
      sponsored.after.treasury - sponsored.before.treasury === sponsored.fee - policy.networkFee * 2
    ])
    checks.push([
      'sponsored seller received the price less the receipt rent and the fee',
      sponsored.after.seller - sponsored.before.seller === amount - receiptRent - sponsored.fee
    ])
    checks.push(['self-sponsored payer paid exactly the price', own.after.buyer - own.before.buyer === -amount])
    checks.push(['self-sponsored sponsor accounted for its own signature', own.after.sponsor - own.before.sponsor === -amount])
    checks.push([
      'self-sponsored treasury kept fee - network',
      own.after.treasury - own.before.treasury === own.fee - policy.networkFee
    ])
    checks.push([
      'self-sponsored seller received the price less the receipt rent and the fee',
      own.after.seller - own.before.seller === amount - receiptRent - own.fee
    ])
    for (const [label, ok] of checks) {
      console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}`)
    }
    if (checks.some(([, ok]) => !ok)) process.exit(1)
    console.log('\nsponsored settle    payer', sponsored.after.buyer - sponsored.before.buyer, 'relayer', sponsored.after.sponsor - sponsored.before.sponsor)
    console.log('self-sponsored      payer', own.after.buyer - own.before.buyer)
  } finally {
    await rm(dir, { recursive: true, force: true })
    void SIGNATURE_FEE
  }
}

await main()
