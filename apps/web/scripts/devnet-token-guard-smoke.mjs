#!/usr/bin/env node
// On-chain check of the two token guards the program enforces: a token budget
// closes only against its own mint, and only its canonical vault can be paid
// from or closed. The offline harness proves both against the built artifact;
// this proves the *deployed* program, which is what a buyer's transaction
// actually reaches.
//
// It sets up a throwaway mint, a token budget and a decoy account, then sends the
// three rejections and reports the code each one fails with. The floats the
// budget opens with are printed too: their amounts are the deployed program's
// constants, so one run says both that the guards are live and which build is.
//
//   node scripts/devnet-token-guard-smoke.mjs
//   node scripts/devnet-token-guard-smoke.mjs --cluster devnet --keypair ~/.config/solana/id.json
//
// Needs the `spl-token` CLI for mints and token accounts, and a funded keypair
// (devnet only, by default) — it pays for everything it creates, sweeps the
// session key's float back to the wallet, and leaves no budget open.
//
// The program's accounts come from the checked-in IDL, so an instruction change
// that the IDL follows cannot leave this script sending an old account list.
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
const RENT_SYSVAR_ID = address('SysvarRent111111111111111111111111111111111')

/** The codes the program answers the two rejections with. */
const MINT_MISMATCH = 6008
const NOT_ASSOCIATED_TOKEN_ACCOUNT = 2009
/** 8 + 32 + 32 + 32 + 8 + 8 + 8 + 1, and a token account's size. */
const TOKEN_BUDGET_SPACE = 129
const TOKEN_ACCOUNT_SPACE = 165
/** One signature's base fee: what a session key keeps to sweep its own float. */
const SESSION_SIGNATURE_FEE = 5_000

const encoder = new TextEncoder()
/** Addresses are base58 strings; as a seed they are their bytes, not their text. */
const bytes = getAddressEncoder()
const seeds = {
  tokenBudget: (buyer, mint) => [encoder.encode('token-budget'), bytes.encode(buyer), bytes.encode(mint)],
  receipt: payment => [encoder.encode('payment-receipt'), bytes.encode(payment)],
  tokenPayment: (seller, reference, mint, amount) => [
    encoder.encode('token-payment'),
    bytes.encode(seller),
    bytes.encode(reference),
    bytes.encode(mint),
    u64(amount)
  ],
  config: () => [encoder.encode('config')],
  treasury: () => [encoder.encode('treasury')]
}

function fail(message) {
  console.error(`error: ${message}`)
  process.exit(1)
}

/** A rejection in words: the message, plus any logs the node attached. */
function describe(rejection) {
  if (!rejection) return 'no reason given'
  if (rejection instanceof Error) {
    const logs = rejection.context?.logs
    return `${rejection.message}${Array.isArray(logs) ? `\n  ${logs.join('\n  ')}` : ''}`
  }
  const text = JSON.stringify(rejection, (_key, value) => (typeof value === 'bigint' ? value.toString() : value))
  return text ?? String(rejection)
}

/**
 * The program's own error code in a failed transaction's error: the node wraps it
 * as `{ InstructionError: [index, { Custom: code }] }`, and the code arrives as a
 * bigint, so reading it is not a string match.
 */
function customCode(seen) {
  if (!seen || typeof seen !== 'object') return null
  if ('Custom' in seen) return Number(seen.Custom)
  const instruction = seen.InstructionError
  if (Array.isArray(instruction)) return customCode(instruction[1])
  if (Array.isArray(seen.InstructionError)) return customCode(seen.InstructionError[1])
  return null
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

/** The cluster's public RPC, unless one is named: the same default the app uses. */
function endpointFor(cluster) {
  if (cluster === 'mainnet' || cluster === 'mainnet-beta') return 'https://api.mainnet-beta.solana.com'
  if (cluster === 'devnet') return 'https://api.devnet.solana.com'
  if (cluster === 'testnet') return 'https://api.testnet.solana.com'
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

/** `system_program::transfer`. */
function transfer(from, to, lamports) {
  return {
    programAddress: SYSTEM_PROGRAM_ID,
    accounts: [
      { address: from, role: 3 },
      { address: to, role: 1 }
    ],
    data: concat(new Uint8Array([2, 0, 0, 0]), u64(lamports))
  }
}

/**
 * `system_program::allocate` and `assign`. An account funded by a plain transfer
 * already exists, so it is sized and owned in place rather than created — which
 * keeps every transaction here single-signer: the new key pays its own fee.
 */
function allocate(account, space) {
  return {
    programAddress: SYSTEM_PROGRAM_ID,
    accounts: [{ address: account, role: 3 }],
    data: concat(new Uint8Array([8, 0, 0, 0]), u64(space))
  }
}

function assign(account, owner) {
  return {
    programAddress: SYSTEM_PROGRAM_ID,
    accounts: [{ address: account, role: 3 }],
    data: concat(new Uint8Array([1, 0, 0, 0]), bytes.encode(owner))
  }
}

/** `spl_token::initialize_account`, for an account that is not an ATA. */
function initializeAccount(account, mint, owner) {
  return {
    programAddress: TOKEN_PROGRAM_ID,
    accounts: [
      { address: account, role: 1 },
      { address: mint, role: 0 },
      { address: owner, role: 0 },
      { address: RENT_SYSVAR_ID, role: 0 }
    ],
    data: new Uint8Array([1])
  }
}

/**
 * An instruction of this program, with its accounts placed by name and its
 * arguments appended to the IDL's discriminator. An account the IDL names but
 * this script cannot place throws instead of being sent as a guess.
 */
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
  const workdir = await mkdtemp(join(tmpdir(), 'payvand-smoke-'))
  /** A throwaway keypair on disk, so the bytes are a real ed25519 keypair. */
  async function freshSigner() {
    const path = join(workdir, `${Math.random().toString(36).slice(2)}.json`)
    execFileSync('solana-keygen', ['new', '--no-bip39-passphrase', '--silent', '--force', '-o', path])
    return await createKeyPairSignerFromBytes(Uint8Array.from(JSON.parse(await readFile(path, 'utf8'))))
  }

  /** `spl-token`, for the token accounts the program only consumes. */
  function token(command, ...rest) {
    const out = execFileSync('spl-token', [
      command,
      ...rest,
      '--url',
      endpoint,
      '--fee-payer',
      args.keypair,
      '--output',
      'json'
    ], {
      encoding: 'utf8',
      maxBuffer: 4 * 1024 * 1024
    })
    return JSON.parse(out.slice(out.indexOf('{')))
  }

  async function ataFor(owner, mint) {
    const [ata] = await getProgramDerivedAddress({
      programAddress: ASSOCIATED_TOKEN_PROGRAM_ID,
      seeds: [bytes.encode(owner), bytes.encode(TOKEN_PROGRAM_ID), bytes.encode(mint)]
    })
    return ata
  }

  /** Lamports as a Number: the RPC answers bigint, and every use here is small. */
  async function rent(space) {
    return Number(await rpc.getMinimumBalanceForRentExemption(space).send())
  }

  async function send(signer, instructions, { skipPreflight = false } = {}) {
    const { value: lifetime } = await rpc.getLatestBlockhash().send()
    const message = pipe(
      createTransactionMessage({ version: 0 }),
      m => setTransactionMessageFeePayerSigner(signer, m),
      m => setTransactionMessageLifetimeUsingBlockhash(lifetime, m),
      m => appendTransactionMessageInstructions(instructions, m)
    )
    let signature
    try {
      signature = await rpc
        .sendTransaction(getBase64EncodedWireTransaction(await signTransactionMessageWithSigners(message)), {
          encoding: 'base64',
          skipPreflight
        })
        .send()
    } catch (cause) {
      return { signature: null, rejected: cause }
    }
    for (let attempt = 0; attempt < 90; attempt++) {
      const status = (await rpc.getSignatureStatuses([signature]).send()).value[0]
      if (status?.err) return { signature, err: status.err }
      if (status && (status.confirmationStatus === 'confirmed' || status.confirmationStatus === 'finalized')) {
        return { signature, err: null }
      }
      await new Promise(resolve => setTimeout(resolve, 1_200))
    }
    fail(`no confirmation for ${signature}`)
  }

  /** A send that must land, so a broken setup stops here instead of later. */
  async function must(what, signer, instructions) {
    const outcome = await send(signer, instructions)
    if (outcome.err || outcome.rejected) fail(`${what} failed: ${describe(outcome.err ?? outcome.rejected)}`)
    return outcome.signature
  }

  const results = []
  /** Sends an instruction that must be refused, and records the code it got. */
  async function expectRejection(label, signer, instruction, code) {
    const outcome = await send(signer, [instruction], { skipPreflight: true })
    const seen = outcome.err ?? outcome.rejected
    const got = customCode(seen)
    if (got !== code) {
      fail(`${label}: expected Custom(${code}), got ${got ?? 'none'} (${outcome.signature ?? 'no signature'})\n  ${describe(seen)}`)
    }
    results.push({ label, code, signature: outcome.signature })
    console.log(`rejected  ${label.padEnd(34)} Custom(${code})  ${outcome.signature}`)
  }

  try {
    console.log(`cluster   ${args.cluster} (${endpoint})`)
    console.log(`program   ${program}`)
    console.log(`admin     ${admin.address}`)

    // Two throwaway mints, and the buyer's account in each.
    const mintA = token('create-token', '--decimals', '6').commandOutput.address
    const mintB = token('create-token', '--decimals', '6').commandOutput.address
    const buyerA = await ataFor(admin.address, mintA)
    const buyerB = await ataFor(admin.address, mintB)
    token('create-account', mintA, '--owner', admin.address)
    token('create-account', mintB, '--owner', admin.address)
    token('mint', mintA, '1000000', buyerA)
    console.log(`mint A    ${mintA}`)
    console.log(`mint B    ${mintB}`)

    // The budget itself, with its own session key and its canonical vault.
    const session = await freshSigner()
    const [budget] = await getProgramDerivedAddress({
      programAddress: program,
      seeds: seeds.tokenBudget(admin.address, mintA)
    })
    const vaultA = await ataFor(budget, mintA)
    token('create-account', mintA, '--owner', budget)
    const limit = 500_000
    const before = Number((await rpc.getBalance(admin.address).send()).value)
    const open = await must('open_token_budget', admin, [
      programInstruction(idl, 'open_token_budget', {
        buyer: admin.address,
        budget,
        mint: mintA,
        buyer_token_account: buyerA,
        vault: vaultA,
        session_key: session.address,
        token_program: TOKEN_PROGRAM_ID,
        associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
        system_program: SYSTEM_PROGRAM_ID
      }, u64(limit))
    ])
    console.log(`budget    ${budget}`)
    console.log(`signature open_token_budget   ${open}`)

    // The floats the budget opens with are the deployed program's constants.
    const sessionLamports = Number((await rpc.getBalance(session.address).send()).value)
    const budgetLamports = Number((await rpc.getBalance(budget).send()).value)
    const budgetFloat = budgetLamports - (await rent(TOKEN_BUDGET_SPACE))
    console.log(`float     session ${sessionLamports}  budget ${budgetFloat}`)

    // The policy and the pool, then the treasury's own account for mint A: since
    // the budget path charges the fee, every mint it settles in needs that
    // destination opened once, as a deployment does.
    const [configPda] = await getProgramDerivedAddress({ programAddress: program, seeds: seeds.config() })
    const [treasury] = await getProgramDerivedAddress({ programAddress: program, seeds: seeds.treasury() })
    const treasuryAta = await ataFor(treasury, mintA)
    if (!(await rpc.getAccountInfo(treasuryAta, { encoding: 'base64' }).send()).value) {
      const opened = await must('opening the treasury account', admin, [
        programInstruction(idl, 'open_treasury_token_account', {
          admin: admin.address,
          config: configPda,
          treasury,
          mint: mintA,
          treasury_token_account: treasuryAta,
          token_program: TOKEN_PROGRAM_ID,
          associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
          system_program: SYSTEM_PROGRAM_ID
        })
      ])
      console.log(`signature open_treasury_token_account ${opened}`)
    }

    // Another mint's canonical vault, so every ATA constraint holds and only the
    // budget's own mint can refuse the close.
    const vaultB = await ataFor(budget, mintB)
    token('create-account', mintB, '--owner', budget)

    await expectRejection(
      'close_token_budget, wrong mint',
      admin,
      programInstruction(idl, 'close_token_budget', {
        buyer: admin.address,
        budget,
        mint: mintB,
        vault: vaultB,
        buyer_token_account: buyerB,
        token_program: TOKEN_PROGRAM_ID
      }),
      MINT_MISMATCH
    )

    // A token account with the vault's mint and authority but not its address: a
    // place to strand the budget's tokens, not a vault.
    const decoy = await freshSigner()
    const decoyRent = await rent(TOKEN_ACCOUNT_SPACE)
    await must('funding the decoy', admin, [transfer(admin.address, decoy.address, decoyRent + 10_000)])
    await must('creating the decoy', decoy, [
      allocate(decoy.address, TOKEN_ACCOUNT_SPACE),
      assign(decoy.address, TOKEN_PROGRAM_ID),
      initializeAccount(decoy.address, mintA, budget)
    ])
    console.log(`decoy     ${decoy.address}`)

    const seller = await freshSigner()
    const sellerAta = await ataFor(seller.address, mintA)
    token('create-account', mintA, '--owner', seller.address)

    const amount = 100_000
    const reference = (await freshSigner()).address
    const [payment] = await getProgramDerivedAddress({
      programAddress: program,
      seeds: seeds.tokenPayment(seller.address, reference, mintA, amount)
    })
    const [receipt] = await getProgramDerivedAddress({ programAddress: program, seeds: seeds.receipt(payment) })
    // The policy, the pool that funds the records, and the fee's destination: the
    // three accounts revision 12 added to the silent path.
    const pay = vault => programInstruction(idl, 'pay_from_token_budget', {
      session_key: session.address,
      budget,
      vault,
      mint: mintA,
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
    }, concat(u64(amount), new Uint8Array([12, 0, 0, 0]), encoder.encode('smoke guards')))

    // The session key pays, as it does in the client; its float covers the fee.
    await expectRejection('pay_from_token_budget, decoy vault', session, pay(decoy.address), NOT_ASSOCIATED_TOKEN_ACCOUNT)
    await expectRejection(
      'close_token_budget, decoy vault',
      admin,
      programInstruction(idl, 'close_token_budget', {
        buyer: admin.address,
        budget,
        mint: mintA,
        vault: decoy.address,
        buyer_token_account: buyerA,
        token_program: TOKEN_PROGRAM_ID
      }),
      NOT_ASSOCIATED_TOKEN_ACCOUNT
    )

    // Nothing the three refusals touched moved: the real vault still holds the
    // limit, the decoy and the seller are empty, and the budget is still open.
    const vaultBalance = Number((await rpc.getTokenAccountBalance(vaultA).send()).value.amount)
    const decoyBalance = Number((await rpc.getTokenAccountBalance(decoy.address).send()).value.amount)
    const sellerBalance = Number((await rpc.getTokenAccountBalance(sellerAta).send()).value.amount)
    const budgetAccount = (await rpc.getAccountInfo(budget, { encoding: 'base64' }).send()).value
    if (vaultBalance !== limit) fail(`the vault should still hold ${limit}, holds ${vaultBalance}`)
    if (decoyBalance !== 0) fail(`the decoy should be empty, holds ${decoyBalance}`)
    if (sellerBalance !== 0) fail(`the seller should not have been paid, holds ${sellerBalance}`)
    if (!budgetAccount) fail('the budget should still be open')
    const after = Number((await rpc.getBalance(admin.address).send()).value)
    console.log(`intact    vault ${vaultBalance}  decoy ${decoyBalance}  seller ${sellerBalance}  budget open`)
    console.log(`setup cost this wallet paid: ${before - after} lamports`)

    // The session key's float goes home before the budget closes. Closing
    // returns the budget's own lamports to the buyer, but the float parked on
    // the session key is only reachable with that key's signature — and this is
    // the last moment this script holds it. The key pays its own signature, as
    // the app's `sweepSessionFloat` does, so it is emptied rather than left
    // holding 30,000,000 lamports the run then discards.
    const sessionFloat = Number((await rpc.getBalance(session.address, { commitment: 'confirmed' }).send()).value)
    const swept = sessionFloat - SESSION_SIGNATURE_FEE
    if (swept > 0) {
      const sweep = await must('sweeping the session float', session, [
        transfer(session.address, admin.address, swept)
      ])
      console.log(`signature sweep_session_float    ${sweep}  (${swept} lamports home)`)
    }

    // Closed on the way out, so the run leaves no budget behind.
    const close = await must('closing the budget', admin, [
      programInstruction(idl, 'close_token_budget', {
        buyer: admin.address,
        budget,
        mint: mintA,
        vault: vaultA,
        buyer_token_account: buyerA,
        token_program: TOKEN_PROGRAM_ID
      })
    ])
    console.log(`signature close_token_budget  ${close}`)

    console.log('\nthe deployed program enforces both guards:')
    for (const result of results) console.log(`  ${result.label} -> Custom(${result.code}) ${result.signature}`)
  } finally {
    await rm(workdir, { recursive: true, force: true })
  }
}

await main()
