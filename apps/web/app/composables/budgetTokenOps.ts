import type { Address, Instruction } from '@vue-solana/nuxt/kit'
import {
  PAYMENT_RECEIPT_SPACE,
  TOKEN_ACCOUNT_SPACE,
  TOKEN_PAYMENT_SPACE
} from '~/utils/invoiceCost'
import {
  buildInstruction,
  deriveAssociatedTokenAddress,
  deriveConfigPda,
  derivePaymentReceiptPda,
  deriveTreasuryPda,
  deriveTreasuryTokenAccount
} from './usePaymentProgram'
import { peekSessionSigner } from './useSessionKey'
import {
  TOKEN_BUDGET_SPACE,
  deriveTokenBudgetPda,
  tokenFloatNeed,
  tokenFloatTopUp,
  tokenOpenCost,
  tokenRemainingThisMonth,
  transferLamportsInstruction,
  type TokenBudgetState
} from './budgetMath'
import { tokenVault } from './budgetAccounts'
import { submitWithSession } from './budgetTransactions'

export function createTokenBudgetOps(ctx: {
  client: ReturnType<typeof useSolanaClient>['client']
  rentMinimum: (space: number) => Promise<number>
  loadTokenBudget: (
    buyer: Address,
    mint: Address,
    options?: { force?: boolean }
  ) => Promise<TokenBudgetState | null>
}) {
  const { client, rentMinimum, loadTokenBudget } = ctx

  /** Why a token invoice cannot ride this allowance, or `null` when it can. */
  async function tokenBudgetObstacle(
    budget: TokenBudgetState,
    amount: number
  ): Promise<string | null> {
    const session = await peekSessionSigner(budget.buyer)
    if (!session || session.address !== budget.sessionKey) {
      return appT('errors.noSessionKey')
    }
    if (amount > tokenRemainingThisMonth(budget)) {
      return appT('errors.tokenLimitUsedUp')
    }
    if (budget.amount < amount) {
      return appT('errors.tokenNearlyEmpty')
    }
    return null
  }

  /**
   * The transfers that refill the token floats for the next payment: one per
   * account that is short, and none when both already cover it.
   */
  async function tokenFloatTopUpInstructions(
    buyer: Address,
    mint: Address
  ): Promise<Instruction[]> {
    const budget = await loadTokenBudget(buyer, mint)
    if (!budget) return []
    const [invoice, receipt, account, budgetRent, session] = await Promise.all([
      rentMinimum(TOKEN_PAYMENT_SPACE),
      rentMinimum(PAYMENT_RECEIPT_SPACE),
      rentMinimum(TOKEN_ACCOUNT_SPACE),
      rentMinimum(TOKEN_BUDGET_SPACE),
      client.rpc.getBalance(budget.sessionKey, { commitment: 'confirmed' }).send()
    ])
    const topUp = tokenFloatTopUp(
      tokenFloatNeed({ invoice, receipt, account }, NETWORK_FEE_LAMPORTS),
      {
        budget: Math.max(0, budget.lamports - budgetRent),
        session: Number(session.value)
      }
    )
    const transfers: Instruction[] = []
    if (topUp.budget > 0) {
      transfers.push(transferLamportsInstruction(buyer, budget.pda, topUp.budget))
    }
    if (topUp.session > 0) {
      transfers.push(transferLamportsInstruction(buyer, budget.sessionKey, topUp.session))
    }
    return transfers
  }

  /** Settle a token invoice from the allowance, with no wallet popup. */
  async function sendSilentTokenPayment(
    buyer: Address,
    params: {
      payment: Address
      seller: Address
      reference: Address
      mint: Address
      amount: number
      memo: string
    }
  ): Promise<string> {
    const session = await peekSessionSigner(buyer)
    if (!session) {
      throw new Error(appT('errors.noSessionKeyHere'))
    }

    const budgetPda = await deriveTokenBudgetPda(buyer, params.mint)
    const instruction = buildInstruction(
      'pay_from_token_budget',
      { amount: params.amount, memo: params.memo },
      {
        session_key: session.address,
        budget: budgetPda,
        vault: await tokenVault(budgetPda, params.mint),
        mint: params.mint,
        seller: params.seller,
        seller_token_account: await deriveAssociatedTokenAddress(params.seller, params.mint),
        payment: params.payment,
        receipt: await derivePaymentReceiptPda(params.payment),
        reference: params.reference,
        config: await deriveConfigPda(),
        treasury: await deriveTreasuryPda(),
        treasury_token_account: await deriveTreasuryTokenAccount(params.mint)
      }
    )

    return await submitWithSession(client, session, instruction)
  }

  /**
   * What one `open_token_budget` signature costs on top of the tokens it moves.
   */
  async function openTokenBudgetCost(
    buyer: Address,
    mint: Address,
    sessionAddress?: Address
  ): Promise<{ rent: number, feeFloat: number, fee: number, total: number }> {
    const [existing, budgetRent, sessionLamports] = await Promise.all([
      loadTokenBudget(buyer, mint),
      rentMinimum(TOKEN_BUDGET_SPACE),
      sessionAddress
        ? client.rpc
            .getBalance(sessionAddress, { commitment: 'confirmed' })
            .send()
            .then(({ value }: { value: bigint }) => Number(value))
        : Promise.resolve(0)
    ])
    return tokenOpenCost({
      exists: Boolean(existing),
      held: existing?.lamports ?? 0,
      budgetRent,
      sessionHeld: sessionLamports,
      fee: NETWORK_FEE_LAMPORTS
    })
  }

  return { tokenBudgetObstacle, tokenFloatTopUpInstructions, sendSilentTokenPayment, openTokenBudgetCost }
}
