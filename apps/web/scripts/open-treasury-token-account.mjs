#!/usr/bin/env node
// Provisions the treasury's token account for a mint: the one call that makes
// invoices in that token payable at all.
//
// `settle_token_payment` sends the protocol's fee to `treasury_token_account`,
// and the treasury is a PDA — only the program can sign for it, so a deployer
// cannot open that account from a wallet. This runs
// `open_treasury_token_account`, which is admin-gated, pays the rent from the
// admin key, and does nothing when the account is already there.
//
//   pnpm treasury:open --mint <mint> --cluster devnet
//   node scripts/open-treasury-token-account.mjs --mint <mint> --cluster mainnet-beta
//
// The accounts come from the checked-in IDL, so an instruction change that the
// IDL follows cannot silently leave this script sending the old account list.
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
const INSTRUCTION = 'open_treasury_token_account'

function fail(message) {
  console.error(`error: ${message}`)
  process.exit(1)
}

function parseArgs(argv) {
  const args = { cluster: 'devnet', keypair: join(homedir(), '.config/solana/id.json') }
  for (let i = 0; i < argv.length; i += 2) {
    const flag = argv[i]
    const value = argv[i + 1]
    if (!value || !flag.startsWith('--')) fail(`unexpected argument ${flag}; use --mint, --cluster, --keypair, --rpc`)
    args[flag.slice(2)] = value
  }
  if (!args.mint) fail('--mint is required (the token the treasury should be able to charge in)')
  return args
}

/** The cluster's public RPC, unless one is named: the same default the app uses. */
function endpointFor(cluster) {
  if (cluster === 'mainnet-beta' || cluster === 'mainnet') return 'https://api.mainnet-beta.solana.com'
  if (cluster === 'devnet') return 'https://api.devnet.solana.com'
  if (cluster === 'testnet') return 'https://api.testnet.solana.com'
  return `https://api.${cluster}.solana.com`
}

async function main() {
  const args = parseArgs(process.argv.slice(2))
  const mint = address(args.mint)
  const rpc = createSolanaRpc(args.rpc ?? endpointFor(args.cluster))

  const idlPath = fileURLToPath(new URL('../app/assets/payvand.idl.json', import.meta.url))
  const idl = JSON.parse(await readFile(idlPath, 'utf8'))
  const program = address(idl.address)
  const definition = idl.instructions.find(instruction => instruction.name === INSTRUCTION)
  if (!definition) fail(`${idlPath} has no ${INSTRUCTION} instruction — run \`pnpm anchor:build\``)

  const secretKey = Uint8Array.from(JSON.parse(await readFile(args.keypair, 'utf8')))
  const admin = await createKeyPairSignerFromBytes(secretKey)
  const [config] = await getProgramDerivedAddress({ programAddress: program, seeds: [new TextEncoder().encode('config')] })
  const [treasury] = await getProgramDerivedAddress({ programAddress: program, seeds: [new TextEncoder().encode('treasury')] })
  const [treasuryTokenAccount] = await getProgramDerivedAddress({
    programAddress: ASSOCIATED_TOKEN_PROGRAM_ID,
    seeds: [getAddressEncoder().encode(treasury), getAddressEncoder().encode(TOKEN_PROGRAM_ID), getAddressEncoder().encode(mint)]
  })

  // The IDL names the accounts and says which of them are writable or signers;
  // this only says where each one lives, so an account added there fails loudly
  // here instead of going unnoticed, and a flag changed there follows through.
  const known = {
    admin: admin.address,
    config,
    treasury,
    mint,
    treasury_token_account: treasuryTokenAccount,
    token_program: TOKEN_PROGRAM_ID,
    associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
    system_program: SYSTEM_PROGRAM_ID
  }
  // `AccountRole`: 0 readonly, 1 writable, 2 readonly signer, 3 writable signer.
  const accounts = definition.accounts.map(({ name, writable, signer }) => {
    const resolved = known[name]
    if (!resolved) fail(`the IDL asks for an account this script cannot place: ${name}`)
    return { address: resolved, role: (signer === true ? 2 : 0) + (writable === true ? 1 : 0) }
  })

  console.log(`cluster   ${args.cluster}`)
  console.log(`mint      ${mint}`)
  console.log(`admin     ${admin.address}`)
  console.log(`account   ${treasuryTokenAccount}`)

  const existing = (await rpc.getAccountInfo(treasuryTokenAccount, { encoding: 'base64' }).send()).value
  if (existing) {
    console.log('already open — nothing to send')
    return
  }

  const { value: latest } = await rpc.getLatestBlockhash().send()
  const message = pipe(
    createTransactionMessage({ version: 0 }),
    m => setTransactionMessageFeePayerSigner(admin, m),
    m => setTransactionMessageLifetimeUsingBlockhash(latest, m),
    m => appendTransactionMessageInstructions([
      { programAddress: program, accounts, data: Uint8Array.from(definition.discriminator) }
    ], m)
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
      console.log('open — invoices in this mint can settle now')
      return
    }
    await new Promise(resolve => setTimeout(resolve, 1_200))
  }
  fail(`no confirmation after a minute: check ${signature}`)
}

await main()
