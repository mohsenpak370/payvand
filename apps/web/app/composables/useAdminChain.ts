import type { ChainOverview } from './useAdminApi'

/**
 * The deployment's accounts and server wallets. Like the payment feed it reads
 * the cluster, so it loads on its own — and only when its tab is opened, so the
 * dashboard's first paint never waits on an RPC.
 */
export function useAdminChain() {
  const { t } = useI18n()
  const { fetchChain } = useAdminApi()

  const chain = shallowRef<ChainOverview | null>(null)
  const chainLoading = shallowRef(false)
  const chainFailure = shallowRef('')

  async function loadChain() {
    chainFailure.value = ''
    chainLoading.value = true
    try {
      chain.value = await fetchChain()
    } catch (error) {
      chainFailure.value = apiErrorMessage(error, t('admin.chain.loadFailure'))
    } finally {
      chainLoading.value = false
    }
  }

  return { chain, chainLoading, chainFailure, loadChain }
}
