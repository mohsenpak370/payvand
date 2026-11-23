import type { Address } from '@vue-solana/nuxt/kit'
import type { ComputedRef, Ref } from 'vue'
import type { TokenBudgetState } from './budgetMath'
import { potNeedsTopUp } from './budgetMath'
import { mintForCurrency } from './usePaymentProgram'
import { peekSessionSigner } from './useSessionKey'
import type { BudgetPageState } from './useBudgetPageState'

/** The one allowance the card is showing, whichever currency holds it. */
export interface BudgetAllowanceView {
  /** This month's ceiling, in the currency's base units. */
  limit: number
  /** What the ceiling leaves, in the same units, before the pot is considered. */
  remaining: number
  /** What a payment can draw on now: the remaining ceiling, capped by the pot. */
  spendable: number
  /** What the pot or vault holds right now. */
  pot: number
  /** The month rolled over under this allowance: the counter reset, no money moved. */
  rolledOver: boolean
  /** The pot is low enough that a top-up would change what can be paid. */
  needsTopUp: boolean
  /** The key that pays from it silently. */
  sessionKey: Address
}

/**
 * Reads every allowance a buyer can have — SOL and each stablecoin — and
 * derives the one on screen.
 *
 * The card follows the currency switch, so a single selection drives both the
 * figures and which account is read back after a mutation. Reads are forced
 * once per refresh: the account cache serves the quote's repeated asks, but a
 * signature that just landed must not be answered from a minute-old read.
 */
export function useBudgetAllowances(deps: {
  state: BudgetPageState
  budgetApi: ReturnType<typeof useBudget>
  publicKey: Ref<Address | null>
}): {
  activeMint: ComputedRef<Address | null>
  tokenBudget: ComputedRef<TokenBudgetState | null>
  view: ComputedRef<BudgetAllowanceView | null>
  foreignDevice: ComputedRef<boolean>
  liveSessionKeys: ComputedRef<Address[]>
  refillNeeded: ComputedRef<number>
  refresh: () => Promise<void>
  refreshRecoverableFloat: () => Promise<void>
} {
  const { state, budgetApi, publicKey } = deps
  const {
    activeCurrency, budget, budgetPot, tokenBudgets, deviceKeyAddress,
    recoverableFloat, budgetsRead, currencies, cluster, failure
  } = state
  const { t } = useI18n()

  const activeMint = computed<Address | null>(() =>
    activeCurrency.value === 'sol' ? null : mintForCurrency(activeCurrency.value, cluster)
  )

  const tokenBudget = computed(() => tokenBudgets.value[activeCurrency.value] ?? null)

  /** The allowance on screen, whichever currency it is held in. */
  const view = computed<BudgetAllowanceView | null>(() => {
    const now = currentMonthIndex()
    if (activeCurrency.value === 'sol') {
      if (!budget.value) return null
      return figureOut(
        budget.value.monthlyLimit,
        budgetApi.remainingThisMonth(budget.value),
        budgetPot.value,
        budget.value,
        now
      )
    }
    const held = tokenBudget.value
    if (!held) return null
    return figureOut(
      held.monthlyLimit,
      budgetApi.tokenRemainingThisMonth(held),
      held.amount,
      held,
      now
    )
  })

  /**
   * True when the allowance on screen is signed by a key this device does not
   * hold: another installation opened it, or this browser was never set up.
   */
  const foreignDevice = computed(
    () => Boolean(view.value && view.value.sessionKey !== deviceKeyAddress.value)
  )

  /**
   * The figures the summary shows from one account's ceiling, what is left of
   * it and what the pot holds. `spendable` is the pair's minimum: the month can
   * reset the ceiling without the pot gaining a lamport.
   */
  function figureOut(
    limit: number,
    remaining: number,
    pot: number,
    account: { monthIndex: number, sessionKey: Address },
    now: number
  ): BudgetAllowanceView {
    const spendable = Math.min(remaining, pot)
    const rolledOver = account.monthIndex !== now
    return {
      limit,
      remaining,
      spendable,
      pot,
      rolledOver,
      needsTopUp: potNeedsTopUp(limit, pot, spendable, rolledOver),
      sessionKey: account.sessionKey
    }
  }

  /**
   * Every session key an allowance on this page names. A key of ours that is not
   * in here no longer pays anything, so its float is ours to take back.
   */
  const liveSessionKeys = computed(() =>
    [budget.value?.sessionKey, ...Object.values(tokenBudgets.value).map(entry => entry?.sessionKey)]
      .filter((key): key is Address => Boolean(key))
  )

  /** What it would take to put the pot back to the full monthly limit. */
  const refillNeeded = computed(() => {
    const current = view.value
    return current ? Math.max(0, current.limit - current.pot) : 0
  })

  /**
   * A key this device holds that no remaining allowance names is one it was
   * rotated out of, or one whose budget is gone: nothing here can spend its
   * float, so it becomes a visible recover action instead of lost lamports.
   */
  async function refreshRecoverableFloat() {
    const key = deviceKeyAddress.value
    if (!key || !budgetsRead.value || liveSessionKeys.value.includes(key)) {
      recoverableFloat.value = 0
      return
    }
    recoverableFloat.value = await budgetApi.sweepableFloat(key).catch(() => 0)
  }

  async function refresh() {
    budget.value = null
    budgetPot.value = 0
    tokenBudgets.value = {}
    budgetsRead.value = false
    const buyer = publicKey.value
    if (!buyer) {
      deviceKeyAddress.value = null
      recoverableFloat.value = 0
      return
    }
    deviceKeyAddress.value = (await peekSessionSigner(buyer))?.address ?? null
    try {
      const loaded = await budgetApi.loadBudget(buyer, { force: true })
      budget.value = loaded
      if (loaded) budgetPot.value = await budgetApi.availablePot(loaded)

      for (const currency of currencies) {
        if (currency === 'sol') continue
        const mint = mintForCurrency(currency, cluster)
        if (!mint) continue
        tokenBudgets.value[currency] = await budgetApi.loadTokenBudget(buyer, mint, {
          force: true
        })
      }
      budgetsRead.value = true
    } catch (error) {
      failure.value = error instanceof Error ? error.message : t('budget.errors.readFailure')
    }
    await refreshRecoverableFloat()
  }

  watch(publicKey, refresh, { immediate: true })

  return {
    activeMint,
    tokenBudget,
    view,
    foreignDevice,
    liveSessionKeys,
    refillNeeded,
    refresh,
    refreshRecoverableFloat
  }
}
