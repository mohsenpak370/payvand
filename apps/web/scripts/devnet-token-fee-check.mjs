#!/usr/bin/env node
// One real token settle on devnet, twice: the wallet sponsoring itself and a
// funded relayer doing it. It checks the token side of the same fee model the SOL
// check covers — the payer's token balance falls by exactly the price, the
// seller receives the price less the policy's token fee, the treasury's own
// account for the mint gains exactly that fee, and whoever fronted the lamports
// (the two record rents and the signatures) ends whole.
//
//   node scripts/devnet-token-fee-check.mjs --cluster devnet
//   node scripts/devnet-token-fee-check.mjs --relayer ../../.secrets/sponsor.json
//
// The mint is the one `Config` names for USDC, and the buyer is funded from the
// admin's own balance of it, so no mint is created and no policy is touched.
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
  getAddressDecoder,
  getAddressEncoder,
  getBase64EncodedWireTransaction,
  getProgramDerivedAddress,
  pipe,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners
} from '@vue-solana/nuxt/kit'

const TOKEN_PROGRAM_ID = address('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA')
const ASSOCIATED_TOKEN_PROGRAM_ID = address('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL')
const SYSTEM_PROGRAM_ID = address('11111111111111111111111111111111')
const encoder = new TextEncoder()
const bytes = getAddressEncoder()
const addresses = getAddressDecoder()

function fail(message) {
  console.error(`error: ${message}`)
  process.exit(1)
}

function parseArgs(argv) {
  const args = { cluster: 'devnet', keypair: join(homedir(), '.config/solana/id.json'), relayer: '.secrets/sponsor.json' }
  for (let i = 0; i < argv.length; i += 1) {
    const flag = argv[i]
    if (!flag.startsWith('--')) fail(`unexpected argument ${flag}; use --cluster, --keypair, --relayer, --rpc, --skip-self`)
    const value = argv[i + 1]
    if (value === undefined || value.startsWith('--')) {
      args[flag.slice(2)] = true
      continue
    }
    args[flag.slice(2)] = value
    i += 1
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
  let relayer = null
  try {
    relayer = await createKeyPairSignerFromBytes(Uint8Array.from(JSON.parse(await readFile(args.relayer, 'utf8'))))
  } catch {
    console.log(`note      no relayer at ${args.relayer}; running the self-sponsored path only`)
  }

  async function pda(seeds) {
    const [found] = await getProgramDerivedAddress({ programAddress: program, seeds })
    return found
  }

  async function ata(owner, mint) {
    const [found] = await getProgramDerivedAddress({
      programAddress: ASSOCIATED_TOKEN_PROGRAM_ID,
      seeds: [bytes.encode(owner), bytes.encode(TOKEN_PROGRAM_ID), bytes.encode(mint)]
    })
    return found
  }

  const config = await pda([encoder.encode('config')])
  const treasury = await pda([encoder.encode('treasury')])

  const info = (await rpc.getAccountInfo(config, { encoding: 'base64' }).send()).value
  if (!info) fail(`no config account at ${config}`)
  const raw = Uint8Array.from(atob(info.data[0]), char => char.charCodeAt(0))
  if (raw.length !== 184) fail(`config is ${raw.length} bytes: this check wants the v4 layout (184)`)
  const view = new DataView(raw.buffer, raw.byteOffset, raw.byteLength)
  const mint = addresses.decode(raw.slice(76, 108))
  const policy = {
    feeBpsBelow: view.getUint16(40, true),
    feeBpsAbove: view.getUint16(42, true),
    tokenTier: Number(view.getBigUint64(52, true)),
    flatUsdc: Number(view.getBigUint64(60, true)),
    networkFee: Number(view.getBigUint64(140, true)),
    networkFeeUsdc: Number(view.getBigUint64(148, true))
  }

  const treasuryToken = await ata(treasury, mint)

  async function lamports(account) {
    return Number((await rpc.getBalance(account).send()).value)
  }

  async function tokenBalance(account) {
    const { value } = await rpc.getTokenAccountBalance(account).send()
    return Number(value.amount)
  }

  async function exists(account) {
    return Boolean((await rpc.getAccountInfo(account, { encoding: 'base64' }).send()).value)
  }

  async function freshSigner(dir, name) {
    const path = join(dir, `${name}.json`)
    execFileSync('solana-keygen', ['new', '--no-bip39-passphrase', '--silent', '-o', path], { encoding: 'utf8' })
    return createKeyPairSignerFromBytes(Uint8Array.from(JSON.parse(await readFile(path, 'utf8'))))
  }

  async function sendChecked(feePayer, signers, instructions) {
    const { value: lifetime } = await rpc.getLatestBlockhash().send()
    const message = addSignersToTransactionMessage(
      signers,
      pipe(
        createTransactionMessage({ version: 0 }),
        m => setTransactionMessageFeePayerSigner(feePayer, m),
        m => setTransactionMessageLifetimeUsingBlockhash(lifetime, m),
        m => appendTransactionMessageInstructions(instructions, m)
      )
    )
    const wire = getBase64EncodedWireTransaction(await signTransactionMessageWithSigners(message))
    const simulation = await rpc.simulateTransaction(wire, { encoding: 'base64', sigVerify: false, replaceRecentBlockhash: true }).send()
    if (simulation.value.err) fail(`simulation refused it: ${describe(simulation.value.err)}\n  ${(simulation.value.logs ?? []).join('\n  ')}`)
    const signature = await rpc.sendTransaction(wire, { encoding: 'base64' }).send()
    for (let attempt = 0; attempt < 90; attempt++) {
      const status = (await rpc.getSignatureStatuses([signature]).send()).value[0]
      if (status?.err) fail(`transaction ${signature} failed: ${describe(status.err)}`)
      if (status && (status.confirmationStatus === 'confirmed' || status.confirmationStatus === 'finalized')) return signature
      await new Promise(resolve => setTimeout(resolve, 1_200))
    }
    fail(`no confirmation for ${signature}`)
  }

  /** `fee_for` on the token path: the boundary rate, floored at the flat plus this path's signatures. */
  function feeForToken(amount, signatures) {
    const below = Math.min(amount, policy.tokenTier)
    const rate = Math.floor((below * policy.feeBpsBelow + (amount - below) * policy.feeBpsAbove) / 10_000)
    return Math.max(rate, policy.flatUsdc + policy.networkFeeUsdc * signatures)
  }

  const dir = await mkdtemp(join(tmpdir(), 'payvand-token-'))
  try {
    console.log(`cluster   ${args.cluster} (${endpoint})`)
    console.log(`mint      ${mint} (from Config)`)
    console.log(`policy    ${policy.feeBpsBelow}/${policy.feeBpsAbove} bps, flat ${policy.flatUsdc}, network ${policy.networkFeeUsdc} per signature`)

    if (!(await exists(treasuryToken))) {
      fail(`the treasury has no account for ${mint}; run \`node scripts/open-treasury-token-account.mjs --mint ${mint} --cluster ${args.cluster}\` first`)
    }

    const amount = 5_000_000
    const buyer = await freshSigner(dir, 'buyer')
    const seller = await freshSigner(dir, 'seller')
    const buyerTokens = await ata(buyer.address, mint)
    const sellerTokens = await ata(seller.address, mint)
    const adminTokens = await ata(admin.address, mint)

    // Token accounts can be opened by anyone, so the admin pays for these and
    // then hands the buyer exactly the price.
    for (const account of [buyerTokens, sellerTokens]) {
      execFileSync('spl-token', [
        'create-account', mint, '--owner', account === buyerTokens ? buyer.address : seller.address,
        '--fee-payer', args.keypair, '--url', endpoint
      ], { encoding: 'utf8', stdio: 'pipe' })
    }
    execFileSync('spl-token', [
      // `spl-token transfer` counts in whole tokens, the program in base units.
      'transfer', mint, String(amount / 1_000_000), buyer.address, '--from', adminTokens,
      '--owner', args.keypair, '--fee-payer', args.keypair, '--url', endpoint,
      '--fund-recipient', '--allow-unfunded-recipient'
    ], { encoding: 'utf8', stdio: 'pipe' })

    // The self-sponsored path has the wallet front both record rents and its own
    // signature for the length of the transaction, all repaid in the same one, so
    // it has to hold at least that much to start.
    await sendChecked(admin, [admin], [{
      programAddress: SYSTEM_PROGRAM_ID,
      accounts: [
        { address: admin.address, role: 3 },
        { address: buyer.address, role: 1 }
      ],
      data: concat(new Uint8Array([2, 0, 0, 0]), u64(5_000_000))
    }])

    const checks = []
    /** Each leg starts from the same place: the buyer holds exactly one price. */
    async function topUp() {
      execFileSync('spl-token', [
        'transfer', mint, String(amount / 1_000_000), buyer.address, '--from', adminTokens,
        '--owner', args.keypair, '--fee-payer', args.keypair, '--url', endpoint, '--fund-recipient'
      ], { encoding: 'utf8', stdio: 'pipe' })
      await sendChecked(admin, [admin], [{
        programAddress: SYSTEM_PROGRAM_ID,
        accounts: [
          { address: admin.address, role: 3 },
          { address: buyer.address, role: 1 }
        ],
        data: concat(new Uint8Array([2, 0, 0, 0]), u64(5_000_000))
      }])
    }

    async function settle({ name, sponsor, signatures }) {
      await topUp()
      const reference = (await freshSigner(dir, `ref-${name.replace(/\s+/g, '-')}`)).address
      const payment = await pda([
        encoder.encode('token-payment'),
        bytes.encode(seller.address),
        bytes.encode(reference),
        bytes.encode(mint),
        u64(amount)
      ])
      const receipt = await pda([encoder.encode('payment-receipt'), bytes.encode(payment)])

      const before = {
        buyerTokens: await tokenBalance(buyerTokens),
        sellerTokens: await tokenBalance(sellerTokens),
        treasuryTokens: await tokenBalance(treasuryToken),
        buyerLamports: await lamports(buyer.address),
        sponsorLamports: await lamports(sponsor.address)
      }

      const instruction = programInstruction(idl, 'settle_token_payment', {
        payer: buyer.address,
        payment,
        seller: seller.address,
        seller_token_account: sellerTokens,
        payer_token_account: buyerTokens,
        mint,
        receipt,
        reference,
        config,
        treasury,
        treasury_token_account: treasuryToken,
        token_program: TOKEN_PROGRAM_ID,
        system_program: SYSTEM_PROGRAM_ID,
        sponsor: sponsor.address
      }, concat(u64(amount), u32LengthPrefixed(`token fee-model ${name}`)))

      // One signature each when the wallet sponsors itself; the same key must
      // not be listed twice in the signer set.
      const signers = sponsor.address === buyer.address ? [buyer] : [buyer, sponsor]
      const signature = await sendChecked(sponsor, signers, [instruction])

      const after = {
        buyerTokens: await tokenBalance(buyerTokens),
        sellerTokens: await tokenBalance(sellerTokens),
        treasuryTokens: await tokenBalance(treasuryToken),
        buyerLamports: await lamports(buyer.address),
        sponsorLamports: await lamports(sponsor.address)
      }
      const fee = feeForToken(amount, signatures)
      console.log(`\n${name}`)
      console.log(`  signature      ${signature}`)
      console.log(`  payer tokens   ${before.buyerTokens} → ${after.buyerTokens} (${after.buyerTokens - before.buyerTokens})`)
      console.log(`  seller tokens  ${before.sellerTokens} → ${after.sellerTokens} (${after.sellerTokens - before.sellerTokens})`)
      console.log(`  treasury       ${before.treasuryTokens} → ${after.treasuryTokens} (${after.treasuryTokens - before.treasuryTokens})`)
      console.log(`  payer lamports ${before.buyerLamports} → ${after.buyerLamports} (${after.buyerLamports - before.buyerLamports})`)
      console.log(`  sponsor lampts ${before.sponsorLamports} → ${after.sponsorLamports} (${after.sponsorLamports - before.sponsorLamports})`)
      console.log(`  policy fee     ${fee} for ${signatures} signature(s)`)

      checks.push([`${name}: payer's tokens fell by exactly the price`, after.buyerTokens - before.buyerTokens === -amount])
      checks.push([`${name}: seller received the price less the fee`, after.sellerTokens - before.sellerTokens === amount - fee])
      checks.push([`${name}: treasury gained exactly the fee`, after.treasuryTokens - before.treasuryTokens === fee])
      checks.push([`${name}: payer's lamports unchanged`, after.buyerLamports - before.buyerLamports === 0])
      checks.push([`${name}: sponsor ended whole`, after.sponsorLamports - before.sponsorLamports === 0])
    }

    if (!args['skip-self']) await settle({ name: 'self-sponsored token', sponsor: buyer, signatures: 1 })
    if (relayer) await settle({ name: 'sponsored token', sponsor: relayer, signatures: 2 })

    for (const [label, ok] of checks) console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}`)
    if (checks.some(([, ok]) => !ok)) process.exit(1)
  } finally {
    await rm(dir, { recursive: true, force: true })
  }
}

try {
  await main()
} catch (cause) {
  console.error(cause?.stack ?? describe(cause))
  process.exit(1)
}
