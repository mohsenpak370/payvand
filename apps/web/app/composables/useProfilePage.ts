import { useProfileAllowance } from './useProfileAllowance'
import { useProfileTransactions } from './useProfileTransactions'
import { useProfileWallet } from './useProfileWallet'

/**
 * The profile page's cards, wired once.
 *
 * This function only coordinates: it owns the linked wallet the other cards
 * follow, passes the history refresh to the wallet actions that change it, and
 * hands the page one flat API. Each card's own fetching and derived state lives
 * in its composable.
 */
export function useProfilePage() {
  const { user, myTransactions } = useAuth()
  const linkedWallet = computed(() => user.value?.wallet ?? null)

  const transactions = useProfileTransactions({ linkedWallet, myTransactions })
  const wallet = useProfileWallet({
    linkedWallet,
    onWalletChanged: transactions.refreshTransactions
  })
  const allowance = useProfileAllowance({
    linkedWallet,
    publicKey: wallet.publicKey
  })

  return {
    user,
    ...wallet,
    ...allowance,
    ...transactions
  }
}
