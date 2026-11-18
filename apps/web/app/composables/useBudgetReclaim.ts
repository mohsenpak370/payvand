import type { Address } from '@vue-solana/nuxt/kit'
import type { ComputedRef, Ref } from 'vue'
import { deriveBudgetPda, deriveTokenBudgetPda } from './budgetMath'
import {
  buildInstruction,
  buildMemoInstruction,
  compileWireTransaction,
  deriveAssociatedTokenAddress
} from './usePaymentProgram'
import { forgetSessionSigner } from './useSessionKey'
import type { BudgetAllowanceView } from './useBudgetAllowances'
import type { useBudgetMessages } from './useBudgetMessages'
import type { BudgetPageState } from './useBudgetPageState'

/**
 * Takes money back out: closing one currency's allowance, and sweeping the
 * session key's float home when it stopped paying for anything.
 *
 * The session key is only forgotten once no allowance is left to spend with it,
 * and only after its float is back. Every path that drops a key goes through
 * `sweepHomeAndForget`, so none can strand lamports on a key it forgets.
 */
export function useBudgetReclaim(deps: {
  state: BudgetPageState
  messages: ReturnType<typeof useBudgetMessages>
  budgetApi: ReturnType<typeof useBudget>
  client: ReturnType<typeof useSolanaClient>['client']
  execute: ReturnType<typeof useSolanaSignAndSendTransaction>['execute']
  publicKey: Ref<Address | null>
  activeMint: ComputedRef<Address | null>
  view: ComputedRef<BudgetAllowanceView | null>
  liveSessionKeys: ComputedRef<Address[]>
  refresh: () => Promise<void>
  refreshRecoverableFloat: () => Promise<void>
  unitLabel: ComputedRef<string>
}) {
  const { state, messages, budgetApi, client, execute, publicKey } = deps
  const { activeMint, view, liveSessionKeys, refresh, refreshRecoverableFloat, unitLabel } = deps
  const {
    busy, failure, failureTitle, success, budget, tokenBudgets,
    deviceKeyAddress, recoverableFloat, sweeping, budgetsRead
  } = state
  const { recordFailure, describeFailure, resetMessages } = messages
  const { t } = useI18n()

  /**
   * Sweeps this device's float home and only then forgets the key — the one
   * place a session key is dropped, so no path can forget one still holding
   * lamports. Throws with the key and its float intact, which is what leaves a
   * retry.
   */
  async function sweepHomeAndForget(buyer: Address): Promise<number> {
    const { swept } = await budgetApi.sweepSessionFloat(buyer)
    forgetSessionSigner(buyer)
    deviceKeyAddress.value = null
    recoverableFloat.value = 0
    return swept
  }

  /**
   * Sends a stranded float home after the key stopped paying for anything: a
   * rotated-out device, or a budget closed from another one.
   */
  async function recoverFloat() {
    const buyer = publicKey.value
    const key = deviceKeyAddress.value
    if (!buyer || !key || !budgetsRead.value || liveSessionKeys.value.includes(key)) return
    resetMessages()
    sweeping.value = true
    try {
      const swept = await sweepHomeAndForget(buyer)
      success.value = t('budget.success.floatRecovered', { amount: lamportsToSol(swept) })
    } catch (error) {
      recordFailure(error)
    } finally {
      sweeping.value = false
    }
  }

  /**
   * Hands the float back after the last allowance is gone, and says what
   * happened to it: the close worked either way, so the message must not claim
   * everything came back when the float is still on this device's key.
   */
  async function handBackFloat(buyer: Address) {
    try {
      await sweepHomeAndForget(buyer)
      success.value = t('budget.success.closed', { currency: unitLabel.value })
    } catch (error) {
      failureTitle.value = t('budget.errors.floatNotRecovered')
      await refreshRecoverableFloat()
      failure.value = t('budget.errors.floatUnsweptBody', {
        reason: describeFailure(error),
        amount: lamportsToSol(recoverableFloat.value)
      })
    }
  }

  /**
   * Reclaim one currency's pot and its account rent (1 signature). The session
   * key's float follows only when this was the last allowance — the other
   * currencies' budgets still pay with it.
   */
  async function closeBudget() {
    const buyer = publicKey.value
    if (!buyer) return

    resetMessages()
    if (!view.value) {
      failure.value = t('budget.errors.noBudgetToClose')
      return
    }
    busy.value = true
    try {
      const mint = activeMint.value
      const instruction = mint
        ? buildInstruction(
            'close_token_budget',
            {},
            {
              buyer,
              budget: await deriveTokenBudgetPda(buyer, mint),
              mint,
              vault: await deriveAssociatedTokenAddress(
                await deriveTokenBudgetPda(buyer, mint),
                mint
              ),
              buyer_token_account: await deriveAssociatedTokenAddress(buyer, mint)
            }
          )
        : buildInstruction('close_budget', {}, { budget: await deriveBudgetPda(buyer), buyer })
      const memo = buildMemoInstruction(
        `Payvand: close ${unitLabel.value} budget and reclaim all funds`,
        buyer
      )
      const wire = await compileWireTransaction({
        instruction: [instruction, memo],
        payer: buyer,
        rpc: client.rpc
      })
      await budgetApi.preflight(wire)
      await execute(wire, {
        confirm: true,
        confirmation: { commitment: 'confirmed', timeoutMs: 120_000 }
      })
      await refresh()

      // The session key is only forgotten once no allowance is left to spend
      // with it — the other currencies' budgets still need it — and only after
      // its float is back, which `handBackFloat` does and reports. An unreadable
      // budget list answers neither question, so the key and its float stay put.
      if (!budgetsRead.value) {
        failure.value = t('budget.errors.readFailure')
        return
      }
      const anyLeft
        = Boolean(budget.value) || Object.values(tokenBudgets.value).some(entry => Boolean(entry))
      if (anyLeft) {
        success.value = t('budget.success.closed', { currency: unitLabel.value })
      } else {
        await handBackFloat(buyer)
      }
    } catch (error) {
      recordFailure(error)
    } finally {
      busy.value = false
    }
  }

  return { closeBudget, recoverFloat }
}
