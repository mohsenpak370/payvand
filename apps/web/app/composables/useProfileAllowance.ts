import type { Address } from '@vue-solana/nuxt/kit'
import type { Ref } from 'vue'
import type { BudgetState } from './useBudget'

interface UseProfileAllowanceOptions {
  linkedWallet: Readonly<Ref<string | null>>
  publicKey: Readonly<Ref<Address | null>>
}

/**
 * Reads the monthly allowance for the wallet that owns the account's payments.
 * The allowance is public on-chain state, so no browser connection is needed;
 * only a fresh signup with nothing linked falls back to the connected wallet.
 */
export function useProfileAllowance({
  linkedWallet,
  publicKey
}: UseProfileAllowanceOptions) {
  const { t } = useI18n()
  const budgetApi = useBudget()

  const budget = ref<BudgetState | null>(null)
  const budgetPot = ref(0)
  const budgetLoading = ref(false)
  const budgetError = ref('')

  // The API stores the linked wallet as a validated base58 address.
  const allowanceAddress = computed<Address | null>(
    () => (linkedWallet.value as Address | null) ?? publicKey.value
  )

  const spentThisMonth = computed(() => {
    const state = budget.value
    if (!state) return 0
    return state.monthIndex === currentMonthIndex() ? state.spent : 0
  })

  const remaining = computed(() =>
    budget.value
      ? Math.max(0, budget.value.monthlyLimit - spentThisMonth.value)
      : 0
  )

  const spentPercent = computed(() => {
    const limit = budget.value?.monthlyLimit ?? 0
    if (!limit) return 0
    return Math.min(100, Math.round((spentThisMonth.value / limit) * 100))
  })

  async function refreshBudget() {
    budget.value = null
    budgetPot.value = 0
    budgetError.value = ''
    const address = allowanceAddress.value
    if (!address) return

    budgetLoading.value = true
    try {
      const loaded = await budgetApi.loadBudget(address, { force: true })
      budget.value = loaded
      if (loaded) budgetPot.value = await budgetApi.availablePot(loaded)
    } catch (error) {
      budgetError.value
        = error instanceof Error ? error.message : t('profile.allowanceError')
    } finally {
      budgetLoading.value = false
    }
  }

  watch(allowanceAddress, refreshBudget, { immediate: true })

  return {
    allowanceAddress,
    budget,
    budgetPot,
    budgetLoading,
    budgetError,
    spentThisMonth,
    remaining,
    spentPercent
  }
}
