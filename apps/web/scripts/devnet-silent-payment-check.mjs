#!/usr/bin/env node
// One real silent token payment against the deployed program: a throwaway mint,
// a token budget for the admin's wallet, a session key that signs alone, and a
// seller that holds no SOL. It checks the split revision 12 decided — the seller
// receives `amount - fee_t` and no lamports, the treasury's own account for the
// mint gains exactly `fee_t`, and the budget's `spent` rises by `amount`.
//
//   node scripts/devnet-silent-payment-check.mjs
//   node scripts/devnet-silent-payment-check.mjs --cluster devnet --keypair ~/.config/solana/id.json
//   node scripts/devnet-silent-payment-check.mjs --mint 4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU
//
// `--mint` spends the wallet's own balance of that token instead of minting a
// throwaway one. That is how the policy's per-currency flat is exercised on
// chain: the mint has to be the one `Config` names, and only a real USDC/EURC
// mint is.
//
// Needs the `spl-token` CLI for the mint and the buyer's account, and a funded
// keypair (devnet only, by default). The policy is read from the on-chain
// `Config`, so the fee it checks is the fee the deployed program charges. The
// session key's float is swept home and the budget is closed on the way out, so
// nothing it created is left open and no session key keeps its float.
import { execFileSync } from 'node:child_process'
import { readFile, mkdtemp, rm } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import {
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
/** One signature's base fee: what a session key keeps to sweep its own float. */
const SESSION_SIGNATURE_FEE = 5_000

const encoder = new TextEncoder()
const bytes = getAddressEncoder()
const addresses = getAddressDecoder()

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
  if (cluster === 'mainnet' || cluster === 'mainnet-beta') return 'https://api.mainnet-beta.solana.com'
  if (cluster === 'devnet') return 'https://api.devnet.solana.com'
  return `https://api.${cluster}.solana.com`
}

function u64(value) {
  const out = new Uint8Array(8)
  new DataView(out.buffer).setBigUint64(0, BigInt(value), true)
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

/** `system_program::transfer`, built by hand: the system program is not in the IDL. */
function transfer(from, to, lamports) {
  const data = new Uint8Array(12)
  const view = new DataView(data.buffer)
  view.setUint32(0, 2, true)
  view.setBigUint64(4, BigInt(lamports), true)
  return {
    programAddress: SYSTEM_PROGRAM_ID,
    accounts: [
      { address: from, role: 3 },
      { address: to, role: 1 }
    ],
    data
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

/** An instruction of this program, with its accounts placed by name. */
function programInstruction(idl, name, placements, args = new Uint8Array()) {
  const definition = idl.instructions.find(instruction => instruction.name === name)
  if (!definition) fail(`the IDL has no ${name} instruction`)
  const accounts = definition.accounts.map(({ name: accountName, writable, signer }) => {
    const resolved = placements[accountName]
    if (!resolved) fail(`${name} asks for an account this script cannot place: ${accountName}`)
    return { address: resolved, role: (signer === true ? 2 : 0) + (writable === true ? 1 : 0) }
  })
  return {
    programAddress: address(idl.address),
    accounts,
    data: concat(Uint8Array.from(definition.discriminator), args)
  }
}

/** A base64 account's bytes. */
function decode(data) {
  return Uint8Array.from(atob(data), char => char.charCodeAt(0))
}

async function main() {
  const args = parseArgs(process.argv.slice(2))
  const endpoint = args.rpc ?? endpointFor(args.cluster)
  const rpc = createSolanaRpc(endpoint)
  const idlPath = fileURLToPath(new URL('../app/assets/payvand.idl.json', import.meta.url))
  const idl = JSON.parse(await readFile(idlPath, 'utf8'))
  const program = address(idl.address)

  const admin = await createKeyPairSignerFromBytes(
    Uint8Array.from(JSON.parse(await readFile(args.keypair, 'utf8')))
  )
  const workdir = await mkdtemp(join(tmpdir(), 'payvand-silent-'))
  async function freshSigner() {
    const path = join(workdir, `${Math.random().toString(36).slice(2)}.json`)
    execFileSync('solana-keygen', ['new', '--no-bip39-passphrase', '--silent', '--force', '-o', path])
    return await createKeyPairSignerFromBytes(Uint8Array.from(JSON.parse(await readFile(path, 'utf8'))))
  }

  function token(...rest) {
    const out = execFileSync('spl-token', [
      ...rest,
      '--url',
      endpoint,
      '--fee-payer',
      args.keypair,
      '--output',
      'json'
    ], { encoding: 'utf8', maxBuffer: 4 * 1024 * 1024 })
    return JSON.parse(out.slice(out.indexOf('{')))
  }

  async function pda(seeds) {
    const [found] = await getProgramDerivedAddress({ programAddress: program, seeds })
    return found
  }

  async function ataFor(owner, mint) {
    const [ata] = await getProgramDerivedAddress({
      programAddress: ASSOCIATED_TOKEN_PROGRAM_ID,
      seeds: [bytes.encode(owner), bytes.encode(TOKEN_PROGRAM_ID), bytes.encode(mint)]
    })
    return ata
  }

  async function lamports(account) {
    return Number((await rpc.getBalance(account).send()).value)
  }

  async function tokenBalance(account) {
    return Number((await rpc.getTokenAccountBalance(account).send()).value.amount)
  }

  async function send(signer, instructions) {
    const { value: lifetime } = await rpc.getLatestBlockhash().send()
    const message = pipe(
      createTransactionMessage({ version: 0 }),
      m => setTransactionMessageFeePayerSigner(signer, m),
      m => setTransactionMessageLifetimeUsingBlockhash(lifetime, m),
      m => appendTransactionMessageInstructions(instructions, m)
    )
    const signature = await rpc
      .sendTransaction(getBase64EncodedWireTransaction(await signTransactionMessageWithSigners(message)), {
        encoding: 'base64'
      })
      .send()
    for (let attempt = 0; attempt < 90; attempt++) {
      const status = (await rpc.getSignatureStatuses([signature]).send()).value[0]
      if (status?.err) fail(`transaction ${signature} failed: ${describe(status.err)}`)
      if (status && (status.confirmationStatus === 'confirmed' || status.confirmationStatus === 'finalized')) {
        return signature
      }
      await new Promise(resolve => setTimeout(resolve, 1_200))
    }
    fail(`no confirmation for ${signature}`)
  }

  try {
    console.log(`cluster   ${args.cluster} (${endpoint})`)
    console.log(`program   ${program}`)
    console.log(`wallet    ${admin.address}`)

    // The policy the deployed program charges, read from its own `Config`.
    const configPda = await pda([encoder.encode('config')])
    const configInfo = (await rpc.getAccountInfo(configPda, { encoding: 'base64' }).send()).value
    if (!configInfo) fail(`no Config at ${configPda}; the program is not initialized on this cluster`)
    const config = decode(configInfo.data[0])
    const view = new DataView(config.buffer, config.byteOffset, config.byteLength)
    const feeBpsBelow = view.getUint16(40, true)
    const feeBpsAbove = view.getUint16(42, true)
    const solTier = Number(view.getBigUint64(44, true))
    const tokenTier = Number(view.getBigUint64(52, true))
    const flatUsdc = Number(view.getBigUint64(60, true))
    const flatEurc = Number(view.getBigUint64(68, true))
    const usdcMint = addresses.decode(config.subarray(76, 108))
    const eurcMint = addresses.decode(config.subarray(108, 140))
    const feeMode = config[140] === 1 ? 'buyer' : 'seller'
    console.log(
      `config    ${configPda}  fee ${feeBpsBelow}/${feeBpsAbove} bps at ${solTier} lamports`
      + ` and ${tokenTier} base units, mode ${feeMode}`
    )
    if (feeMode !== 'seller') fail('the budget path only settles a seller-paid policy')

    // A throwaway mint, with the buyer's own account funded. `--mint` runs the
    // same flow against a token the policy names instead, spending the wallet's
    // existing balance rather than minting a token no policy could charge a flat
    // on.
    const limit = 500_000
    const mint = args.mint ? address(args.mint) : token('create-token', '--decimals', '6').commandOutput.address
    const buyerAta = await ataFor(admin.address, mint)
    if (args.mint) {
      const buyerAccount = (await rpc.getAccountInfo(buyerAta, { encoding: 'base64' }).send()).value
      if (!buyerAccount) fail(`no token account ${buyerAta} for ${mint} to spend from`)
      const funded = await tokenBalance(buyerAta)
      if (funded < limit) fail(`the budget needs ${limit} base units, ${buyerAta} holds ${funded}`)
      console.log(`mint      ${mint}  (existing, buyer holds ${funded} base units)`)
    } else {
      token('create-account', mint, '--owner', admin.address)
      token('mint', mint, String(limit), buyerAta)
      console.log(`mint      ${mint}`)
    }

    // The treasury's account for this mint: opened by the admin if it is the
    // first invoice in this mint, which is also the account the fee lands in.
    const treasury = await pda([encoder.encode('treasury')])
    const treasuryAta = await ataFor(treasury, mint)
    const before = (await rpc.getAccountInfo(treasuryAta, { encoding: 'base64' }).send()).value
    if (!before) {
      const opened = await send(admin, [
        programInstruction(idl, 'open_treasury_token_account', {
          admin: admin.address,
          config: configPda,
          treasury,
          mint,
          treasury_token_account: treasuryAta,
          token_program: TOKEN_PROGRAM_ID,
          associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
          system_program: SYSTEM_PROGRAM_ID
        })
      ])
      console.log(`signature open_treasury_token_account ${opened}`)
    }
    const feeBefore = await tokenBalance(treasuryAta)

    // The budget, its vault, and the session key that will pay without the wallet.
    const session = await freshSigner()
    const [budget] = await getProgramDerivedAddress({
      programAddress: program,
      seeds: [encoder.encode('token-budget'), bytes.encode(admin.address), bytes.encode(mint)]
    })
    const vault = await ataFor(budget, mint)
    const opened = await send(admin, [
      programInstruction(idl, 'open_token_budget', {
        buyer: admin.address,
        budget,
        mint,
        buyer_token_account: buyerAta,
        vault,
        session_key: session.address,
        token_program: TOKEN_PROGRAM_ID,
        associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
        system_program: SYSTEM_PROGRAM_ID
      }, u64(limit))
    ])
    console.log(`signature open_token_budget  ${opened}`)
    const sessionLamports = await lamports(session.address)
    console.log(`float     session ${sessionLamports} lamports`)

    // A seller that holds nothing at all: no SOL, no token account.
    const seller = await freshSigner()
    const sellerAta = await ataFor(seller.address, mint)
    const sellerLamportsBefore = await lamports(seller.address)

    const amount = 100_000
    // `fee_for`: the part at or below the token boundary at one rate, the rest
    // at the other, plus this mint's flat if the policy names it.
    const flat = mint === usdcMint ? flatUsdc : mint === eurcMint ? flatEurc : 0
    const below = Math.min(amount, tokenTier)
    const fee = Math.min(
      Math.floor((below * feeBpsBelow + (amount - below) * feeBpsAbove) / 10_000) + flat,
      amount - 1
    )
    const reference = (await freshSigner()).address
    const [payment] = await getProgramDerivedAddress({
      programAddress: program,
      seeds: [
        encoder.encode('token-payment'),
        bytes.encode(seller.address),
        bytes.encode(reference),
        bytes.encode(mint),
        u64(amount)
      ]
    })
    const [receipt] = await getProgramDerivedAddress({
      programAddress: program,
      seeds: [encoder.encode('payment-receipt'), bytes.encode(payment)]
    })

    const paid = await send(session, [
      programInstruction(idl, 'pay_from_token_budget', {
        session_key: session.address,
        budget,
        vault,
        mint,
        seller: seller.address,
        seller_token_account: sellerAta,
        payment,
        receipt,
        reference,
        config: configPda,
        treasury,
        treasury_token_account: treasuryAta,
        token_program: TOKEN_PROGRAM_ID,
        associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
        system_program: SYSTEM_PROGRAM_ID
      }, concat(u64(amount), u32LengthPrefixed('silent, on devnet')))
    ])
    console.log(`signature pay_from_token_budget ${paid}`)

    // The split, and nothing else.
    const sellerTokens = await tokenBalance(sellerAta)
    const feeAfter = await tokenBalance(treasuryAta)
    const sellerLamportsAfter = await lamports(seller.address)
    // `TokenBudget`: buyer, session key, mint, monthly limit, then `spent` at 112.
    const budgetAccount = (await rpc.getAccountInfo(budget, { encoding: 'base64' }).send()).value
    const spent = budgetAccount
      ? Number(new DataView(decode(budgetAccount.data[0]).buffer).getBigUint64(112, true))
      : null
    const sessionAfter = await lamports(session.address)

    console.log(`\nseller    ${seller.address}`)
    console.log(`  tokens  ${sellerTokens}  (expected ${amount - fee})`)
    console.log(`  lamports ${sellerLamportsAfter}  (expected 0)`)
    console.log(`treasury  ${treasuryAta}`)
    console.log(`  tokens  ${feeAfter}  (expected ${feeBefore + fee}, gained ${feeAfter - feeBefore})`)
    console.log(`budget    spent ${spent}  (expected ${amount})`)
    console.log(`session   ${sessionAfter} lamports, down ${sessionLamports - sessionAfter}`)

    if (sellerTokens !== amount - fee) fail(`the seller should hold ${amount - fee}, holds ${sellerTokens}`)
    if (feeAfter - feeBefore !== fee) fail(`the treasury should gain ${fee}, gained ${feeAfter - feeBefore}`)
    if (sellerLamportsBefore !== 0 || sellerLamportsAfter !== 0) {
      fail(`the seller should hold no lamports, holds ${sellerLamportsAfter}`)
    }
    if (spent !== amount) fail(`the budget should have spent ${amount}, spent ${spent}`)

    // The session key's float goes home before the budget closes: closing
    // returns the budget's own lamports to the buyer, but the float on the
    // session key needs that key's signature, and this is the last moment the
    // script holds it. The key pays its own signature, as the app's
    // `sweepSessionFloat` does, so nothing is left on a key about to be
    // discarded.
    const sessionFloat = await lamports(session.address)
    const swept = sessionFloat - SESSION_SIGNATURE_FEE
    if (swept > 0) {
      const sweep = await send(session, [transfer(session.address, admin.address, swept)])
      console.log(`signature sweep_session_float ${sweep}  (${swept} lamports home)`)
    }

    // Closed on the way out, so the run leaves no budget behind.
    const closed = await send(admin, [
      programInstruction(idl, 'close_token_budget', {
        buyer: admin.address,
        budget,
        mint,
        vault,
        buyer_token_account: buyerAta,
        token_program: TOKEN_PROGRAM_ID
      })
    ])
    console.log(`signature close_token_budget ${closed}`)
    console.log('\nthe deployed program split the price as revision 12 decided.')
  } finally {
    await rm(workdir, { recursive: true, force: true })
  }
}

/** Borsh's `String`: a u32 length, then the bytes. */
function u32LengthPrefixed(text) {
  const body = encoder.encode(text)
  const out = new Uint8Array(4 + body.length)
  new DataView(out.buffer).setUint32(0, body.length, true)
  out.set(body, 4)
  return out
}

await main()
