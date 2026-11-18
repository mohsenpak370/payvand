import { address, type Address } from '@vue-solana/nuxt/kit'
import type { ComputedRef, Ref } from 'vue'
import { budgetBaseUnits } from '~/utils/budgetAmount'
import type { BudgetAllowanceView } from './useBudgetAllowances'
import type { BudgetPageState } from './useBudgetPageState'

/** What one signature costs in SOL, row by row, for the quote card. */
export interface BudgetOpenCost {
  rows: { key: string, lamports: number }[]
  total: number
}

/**
 * Prices the limit on screen: what its next signature would move out of the
 * wallet, read from the same accounts the program reads.
 *
 * Every keystroke in the limit field would be a handful of RPC reads, so the
 * quote waits for the typing to pause. A refill, a save or a close also moves
 * it, because the top-up is `limit - pot` — the quote follows the allowance
 * state, not only the field.
 */
export function useBudgetQuote(deps: {
  state: BudgetPageState
  budgetApi: ReturnType<typeof useBudget>
  publicKey: Ref<Address | null>
  activeMint: ComputedRef<Address | null>
  view: ComputedRef<BudgetAllowanceView | null>
}) {
  const { state, budgetApi, publicKey, activeMint, view } = deps
  const { activeCurrency, limitAmount, deviceKeyAddress } = state

  /**
   * The SOL cost of the signature that would apply this limit, row by row. The
   * session-key float comes from `session_fee_float` on chain (the token flows
   * still read the program's constant), so a policy change moves this too.
   */
  const openCost = ref<BudgetOpenCost | null>(null)
  const openCostLoading = ref(false)

  /** The limit being priced, in base units; 0 while the field holds no amount. */
  const limitBaseUnits = computed(() => {
    const units = budgetBaseUnits(activeCurrency.value, Number(limitAmount.value))
    return Number.isFinite(units) && units > 0 ? units : 0
  })

  /** A real limit the chain could not price — as opposed to one still on its way. */
  const openCostMissing = computed(
    () => limitBaseUnits.value > 0 && !openCostLoading.value && !openCost.value
  )

  async function refreshOpenCost() {
    const buyer = publicKey.value
    const limit = limitBaseUnits.value
    if (!buyer || limit <= 0) {
      openCost.value = null
      openCostLoading.value = false
      return
    }
    openCostLoading.value = true
    try {
      const sessionAddress = deviceKeyAddress.value ? address(deviceKeyAddress.value) : undefined
      const mint = activeMint.value
      if (mint) {
        const cost = await budgetApi.openTokenBudgetCost(buyer, mint, sessionAddress)
        openCost.value = {
          rows: [
            { key: 'feeFloat', lamports: cost.feeFloat },
            { key: 'rent', lamports: cost.rent },
            { key: 'fee', lamports: cost.fee }
          ],
          total: cost.total
        }
      } else {
        const cost = await budgetApi.openBudgetCost(buyer, limit, sessionAddress)
        openCost.value = {
          rows: [
            { key: 'topUp', lamports: cost.topUp },
            { key: 'feeFloat', lamports: cost.feeFloat },
            { key: 'rent', lamports: cost.rent },
            { key: 'fee', lamports: cost.fee }
          ],
          total: cost.total
        }
      }
    } catch {
      // A quote that needs the chain and cannot reach it says so, rather than
      // promising a number.
      openCost.value = null
    } finally {
      openCostLoading.value = false
    }
  }

  let costTimer: ReturnType<typeof setTimeout> | undefined

  function scheduleOpenCost() {
    clearTimeout(costTimer)
    costTimer = setTimeout(() => {
      void refreshOpenCost()
    }, 300)
  }

  watch([publicKey, activeMint], () => {
    void refreshOpenCost()
  })
  // Set by `refresh()` once it knows which key this device holds.
  watch(deviceKeyAddress, () => {
    void refreshOpenCost()
  })
  watch(view, () => {
    void refreshOpenCost()
  })
  watch(limitAmount, scheduleOpenCost)
  onBeforeUnmount(() => {
    clearTimeout(costTimer)
  })

  return { openCost, openCostMissing }
}
