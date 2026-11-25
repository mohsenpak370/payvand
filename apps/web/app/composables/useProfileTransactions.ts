import type { Ref } from 'vue'
import type { TransactionsResponse } from './useAuth'

interface UseProfileTransactionsOptions {
  /** The history follows the linked wallet, not whichever one is connected. */
  linkedWallet: Readonly<Ref<string | null>>
  myTransactions: (options?: { scan?: boolean }) => Promise<TransactionsResponse>
}

/** The on-chain history shown in the profile's transactions card. */
export function useProfileTransactions({
  linkedWallet,
  myTransactions
}: UseProfileTransactionsOptions) {
  const { t } = useI18n()

  const transactions = ref<TransactionsResponse | null>(null)
  const txLoading = ref(false)
  const txFailure = ref('')

  /**
   * `scan` is what the refresh button asks for: the server's own rows answer
   * the card, and only a scan can find a payment the API never wrote an invoice
   * for — one from an allowance, or a code another client made.
   */
  async function refreshTransactions(scan = false) {
    if (!linkedWallet.value) {
      transactions.value = null
      return
    }
    txFailure.value = ''
    txLoading.value = true
    try {
      transactions.value = await myTransactions({ scan })
    } catch (error) {
      txFailure.value = apiErrorMessage(error, t('profile.txFailure'))
    } finally {
      txLoading.value = false
    }
  }

  onMounted(() => refreshTransactions())

  return { transactions, txLoading, txFailure, refreshTransactions }
}
