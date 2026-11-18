import type { PaymentsPage } from './useAdminApi'

/**
 * The chain-backed invoice feed. It reads the cluster, so it loads and fails
 * apart from the SQL data: a flaky RPC must not blank the account table.
 */
export function useAdminFeed() {
  const { t } = useI18n()
  const { fetchPayments } = useAdminApi()

  const feed = shallowRef<PaymentsPage | null>(null)
  const feedLoading = shallowRef(false)
  const feedFailure = shallowRef('')

  async function loadFeed() {
    feedFailure.value = ''
    feedLoading.value = true
    try {
      feed.value = await fetchPayments(20)
    } catch (error) {
      feedFailure.value = apiErrorMessage(error, t('admin.feed.loadFailure'))
    } finally {
      feedLoading.value = false
    }
  }

  onMounted(loadFeed)

  return { feed, feedLoading, feedFailure, loadFeed }
}
