import {
  appendTransactionMessageInstruction,
  createTransactionMessage,
  getBase64EncodedWireTransaction,
  getTransactionDecoder,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
  type Address,
  type Instruction,
  type KeyPairSigner
} from '@vue-solana/nuxt/kit'
import { describeProgramErrorCode } from './usePaymentProgram'
import type { compileWireTransaction } from './usePaymentProgram'
import { peekSessionSigner } from './useSessionKey'
import { sweepableLamports, transferLamportsInstruction } from './budgetMath'

/**
 * Runs the compiled transaction through the cluster's simulator (no signature
 * needed) and throws a user-facing message when it would fail.
 */
export async function preflightBudgetTransaction(
  client: ReturnType<typeof useSolanaClient>['client'],
  wire: Awaited<ReturnType<typeof compileWireTransaction>>
): Promise<void> {
  const simulation = await client.rpc
    .simulateTransaction(getBase64EncodedWireTransaction(getTransactionDecoder().decode(wire)), {
      encoding: 'base64',
      commitment: 'confirmed',
      sigVerify: false,
      replaceRecentBlockhash: true
    })
    .send()
  const err = simulation.value.err
  if (err === null) return

  // Kit's RPC hands the custom code back as a bigint — Number() it so the IDL
  // lookup (numeric codes) can match.
  const custom
    = typeof err === 'object'
      && err !== null
      && 'InstructionError' in err
      && Array.isArray(err.InstructionError)
      ? (err.InstructionError[1] as { Custom?: number | bigint } | undefined)?.Custom
      : undefined
  const anchorMessage = custom !== undefined ? describeProgramErrorCode(Number(custom)) : null
  if (anchorMessage) {
    throw new Error(anchorMessage)
  }
  // Anchor's built-in errors (ConstraintSeeds, AccountNotInitialized, …) are
  // not in the IDL table, but their message is in the logs.
  const anchorLog = simulation.value.logs?.find((line: string) => line.includes('Error Message:'))
  const loggedMessage = anchorLog?.match(/Error Message: (.+?)\.?$/)?.[1]
  if (loggedMessage) {
    throw new Error(loggedMessage)
  }
  if (simulation.value.logs?.some((line: string) => line.includes('insufficient lamports'))) {
    throw new Error(appT('errors.solForLimit'))
  }
  throw new Error(appT('errors.rejectedOnChain'))
}

/** Signs one instruction with the session key and waits for it to land. */
export async function submitWithSession(
  client: ReturnType<typeof useSolanaClient>['client'],
  session: KeyPairSigner,
  instruction: Instruction
): Promise<string> {
  const { value: lifetime } = await client.rpc.getLatestBlockhash().send()
  const message = setTransactionMessageLifetimeUsingBlockhash(
    lifetime,
    setTransactionMessageFeePayerSigner(
      session,
      appendTransactionMessageInstruction(
        instruction,
        createTransactionMessage({ version: 0 })
      )
    )
  )

  const signed = await signTransactionMessageWithSigners(message)
  // The wire bytes are base64, and this RPC method does not assume it: the
  // overload without `encoding` is the deprecated base58 one, so omitting it
  // makes the node decode base64 as base58 and reject the send with
  // `invalid base58 encoding` before the transaction is ever simulated.
  const signature = await client.rpc
    .sendTransaction(getBase64EncodedWireTransaction(signed), { encoding: 'base64' })
    .send()

  for (let i = 0; i < 90; i++) {
    const status = (await client.rpc.getSignatureStatuses([signature]).send()).value[0]
    if (status) {
      if (status.err) throw new Error(appT('errors.paymentRejected'))
      return signature
    }
    await new Promise((resolve) => {
      setTimeout(resolve, 1000)
    })
  }
  throw new Error(appT('errors.notConfirmed'))
}

/** Lamports a sweep of this session key would hand back to the buyer. */
export async function sweepableSessionFloat(
  client: ReturnType<typeof useSolanaClient>['client'],
  sessionAddress: Address
): Promise<number> {
  const balance = Number(
    (await client.rpc.getBalance(sessionAddress, { commitment: 'confirmed' }).send()).value
  )
  return sweepableLamports(balance)
}

/**
 * Hands this device's session-key float back to the buyer. Returns `swept: 0`
 * when there is nothing to send, and throws only when the float is still there.
 */
export async function sweepSessionFloat(
  client: ReturnType<typeof useSolanaClient>['client'],
  buyer: Address
): Promise<{ swept: number, signature: string | null }> {
  const session = await peekSessionSigner(buyer)
  if (!session) return { swept: 0, signature: null }
  const swept = await sweepableSessionFloat(client, session.address)
  if (swept <= 0) return { swept: 0, signature: null }
  try {
    const signature = await submitWithSession(
      client,
      session,
      transferLamportsInstruction(session.address, buyer, swept)
    )
    return { swept, signature }
  } catch (error) {
    // A send can land and still time out in the confirmation poll below, and
    // reporting that as a failure would keep a key whose float has already
    // moved home — and show a retry for money that is back. The balance is the
    // truth; only a key that still holds a float failed to sweep.
    const left = await sweepableSessionFloat(client, session.address).catch(() => swept)
    if (left > 0) throw error
    return { swept, signature: null }
  }
}
