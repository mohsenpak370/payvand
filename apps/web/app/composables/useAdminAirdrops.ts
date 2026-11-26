import type { AirdropPage } from './useAdminApi'

export const AIRDROP_PAGE_SIZE = 20

/** The faucet's claim history, paged. Loaded when its tab is first opened. */
export function useAdminAirdrops() {
  const { t } = useI18n()
  const { fetchAirdrops } = useAdminApi()

  const airdrops = shallowRef<AirdropPage | null>(null)
  const airdropsLoading = shallowRef(false)
  const airdropsFailure = shallowRef('')
  const airdropsOffset = shallowRef(0)

  async function loadAirdrops() {
    airdropsFailure.value = ''
    airdropsLoading.value = true
    try {
      airdrops.value = await fetchAirdrops(AIRDROP_PAGE_SIZE, airdropsOffset.value)
    } catch (error) {
      airdropsFailure.value = apiErrorMessage(error, t('admin.airdrops.loadFailure'))
    } finally {
      airdropsLoading.value = false
    }
  }

  function airdropsPrevious() {
    airdropsOffset.value = Math.max(0, airdropsOffset.value - AIRDROP_PAGE_SIZE)
    void loadAirdrops()
  }

  function airdropsNext() {
    airdropsOffset.value += AIRDROP_PAGE_SIZE
    void loadAirdrops()
  }

  return {
    airdrops,
    airdropsLoading,
    airdropsFailure,
    airdropsOffset,
    airdropsPageSize: AIRDROP_PAGE_SIZE,
    loadAirdrops,
    airdropsPrevious,
    airdropsNext
  }
}
