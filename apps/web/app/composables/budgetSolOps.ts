import type { Address } from '@vue-solana/nuxt/kit'
import {
  buildInstruction,
  deriveConfigPda,
  derivePaymentReceiptPda,
  deriveTreasuryPda
} from './usePaymentProgram'
import { peekSessionSigner } from './useSessionKey'
import { BUDGET_SPACE, deriveBudgetPda, remainingThisMonth, type BudgetState } from './budgetMath'
import { submitWithSession } from './budgetTransactions'

export function createSolBudgetOps(ctx: {
  client: ReturnType<typeof useSolanaClient>['client']
  feeConfig: { load: () => Promise<{ sessionFeeFloat: number }> }
  rentMinimum: (space: number) => Promise<number>
  loadBudget: (buyer: Address, options?: { force?: boolean }) => Promise<BudgetState | null>
}) {
  const { client, feeConfig, rentMinimum, loadBudget } = ctx

  /** The pot above its rent-exempt reserve — what invoices are actually paid from. */
  async function availablePot(budget: BudgetState): Promise<number> {
    return budget.lamports - (await rentMinimum(BUDGET_SPACE))
  }

  /**
   * What one `open_budget` signature must move out of the buyer's wallet:
   * the pot top-up, the session key's fee float (read from `Config`), the
   * budget account's rent (only when it does not exist yet) and the transaction
   * fee. Mirrors the transfers the program performs.
   */
  async function openBudgetCost(
    buyer: Address,
    monthlyLimit: number,
    /** The key that will sign; without one the float starts from nothing. */
    sessionAddress?: Address
  ): Promise<{ topUp: number, feeFloat: number, rent: number, fee: number, total: number }> {
    const existing = await loadBudget(buyer)
    const rent = existing ? 0 : await rentMinimum(BUDGET_SPACE)
    const pot = existing ? await availablePot(existing) : 0
    const topUp = Math.max(0, monthlyLimit - pot)
    // A key this device has not made yet holds nothing, so `open_budget` funds
    // the whole float; quoting a brand-new key at the buyer's balance would
    // understate what the signature moves.
    const sessionLamports = sessionAddress
      ? Number(
          (await client.rpc.getBalance(sessionAddress, { commitment: 'confirmed' }).send()).value
        )
      : 0
    const { sessionFeeFloat } = await feeConfig.load()
    const feeFloat = Math.max(0, sessionFeeFloat - sessionLamports)
    const fee = NETWORK_FEE_LAMPORTS
    return { topUp, feeFloat, rent, fee, total: topUp + feeFloat + rent + fee }
  }

  /**
   * Why the next payment cannot ride the budget, or `null` when it can be
   * settled silently with no wallet popup.
   */
  async function silentPaymentObstacle(
    budget: BudgetState,
    amount: number
  ): Promise<string | null> {
    const session = await peekSessionSigner(budget.buyer)
    if (!session || session.address !== budget.sessionKey) {
      return appT('errors.noSessionKey')
    }
    if (amount > remainingThisMonth(budget)) {
      return appT('errors.limitUsedUp')
    }
    // The pot pays exactly the invoice amount: the session key advances both
    // rents and is reimbursed from the pot, the invoice rent comes back through
    // the close, and the treasury refunds the receipt rent to the pot.
    if ((await availablePot(budget)) < amount) {
      return appT('errors.nearlyEmpty')
    }
    return null
  }

  /**
   * Settle an invoice straight from the budget. The session key signs — no
   * wallet is involved, so no popup ever appears. Confirms up to 90s.
   */
  async function sendSilentPayment(
    buyer: Address,
    params: {
      payment: Address
      seller: Address
      reference: Address
      amount: number
      memo: string
    }
  ): Promise<string> {
    const session = await peekSessionSigner(buyer)
    if (!session) {
      throw new Error(appT('errors.noSessionKeyHere'))
    }

    const instruction = buildInstruction(
      'pay_from_budget',
      { amount: params.amount, memo: params.memo },
      {
        session_key: session.address,
        budget: await deriveBudgetPda(buyer),
        payment: params.payment,
        seller: params.seller,
        receipt: await derivePaymentReceiptPda(params.payment),
        reference: params.reference,
        config: await deriveConfigPda(),
        treasury: await deriveTreasuryPda()
      }
    )

    return await submitWithSession(client, session, instruction)
  }

  return { availablePot, openBudgetCost, silentPaymentObstacle, sendSilentPayment }
}
