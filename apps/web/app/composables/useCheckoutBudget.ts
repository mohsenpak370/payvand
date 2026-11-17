import type { Address } from '@vue-solana/nuxt/kit'
import type { ComputedRef, Ref } from 'vue'
import type { BudgetState, TokenBudgetState } from './useBudget'
import type { PaymentOption, PaymentQrPayload } from './usePaymentApi'
import { mintForCurrency } from './usePaymentProgram'
import { peekSessionSigner } from './useSessionKey'

/** The allowance that could pay this invoice, and whether this device can use it. */
export function useCheckoutBudget(deps: {
  payer: () => Address | null
  payload: Ref<PaymentQrPayload | null>
  selected: ComputedRef<PaymentOption>
  payableAmount: ComputedRef<number>
  budget: ReturnType<typeof useBudget>
}) {
  const { payer, payload, selected, payableAmount, budget } = deps

  // Three separate things agreeing: the wallet says *who* pays, the budget says
  // *how much* is allowed, and the session key on this device says *how* to sign
  // without a popup. Missing any one of them is a different screen, not a
  // generic error.
  const budgetState = ref<BudgetState | null>(null)
  const tokenBudgetState = ref<TokenBudgetState | null>(null)
  const budgetObstacle = ref<string | null>(null)
  const sessionReady = ref(false)

  async function refreshBudget() {
    budgetState.value = null
    tokenBudgetState.value = null
    budgetObstacle.value = null
    sessionReady.value = false
    const buyer = payer()
    if (!buyer) return

    const option = selected.value
    try {
      const mint = option.currency === 'sol'
        ? null
        : mintForCurrency(option.currency as 'usdc' | 'eurc', payload.value?.cluster ?? 'devnet')
      if (option.currency !== 'sol' && !mint) return

      if (mint) {
        const state = await budget.loadTokenBudget(buyer, mint)
        tokenBudgetState.value = state
        if (!state) return
        const session = await peekSessionSigner(state.buyer)
        sessionReady.value = Boolean(session && session.address === state.sessionKey)
        budgetObstacle.value = sessionReady.value
          ? await budget.tokenBudgetObstacle(state, payableAmount.value)
          : null
        return
      }

      const state = await budget.loadBudget(buyer)
      budgetState.value = state
      if (!state) return
      const session = await peekSessionSigner(state.buyer)
      sessionReady.value = Boolean(session && session.address === state.sessionKey)
      // Only ask the budget whether it covers this amount once we know it can
      // sign at all — otherwise every amount reports the same missing key.
      budgetObstacle.value = sessionReady.value
        ? await budget.silentPaymentObstacle(state, payableAmount.value)
        : null
    } catch {
      // A flaky RPC read must never block paying — the wallet path still works.
    }
  }

  /** The allowance that would pay in the chosen currency. */
  const activeBudget = computed(() =>
    selected.value.currency === 'sol' ? budgetState.value : tokenBudgetState.value
  )

  /**
   * This browser can settle the invoice with no wallet popup. It says nothing
   * about the invoice or the wallet — those are what the two payment paths add —
   * so the stage can read it as "this device has a way to pay" on its own.
   */
  const canPaySilently = computed(
    () => Boolean(activeBudget.value) && sessionReady.value && !budgetObstacle.value
  )

  return {
    budgetState,
    tokenBudgetState,
    budgetObstacle,
    sessionReady,
    refreshBudget,
    activeBudget,
    canPaySilently
  }
}
