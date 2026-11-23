/**
 * The app's own devnet faucet: one claim per account, sent by the API from its
 * treasury to the wallet the account linked.
 *
 * The amounts are the server's, so the card never restates them from memory —
 * it asks what a claim would send and shows that.
 */
export interface FaucetStatus {
  claimable: boolean
  claimed: boolean
  /** The wallet the tokens would go to, when one is linked. */
  wallet: string | null
  cluster: string
  /** Lamports a claim sends; 1 SOL by default. */
  sol_lamports: number
  /** Base units of each stablecoin a claim sends; 10 tokens by default. */
  token_base_units: number
  /** Why a claim is not possible, when it is not. */
  reason: string | null
}

export interface FaucetClaim {
  signature: string
  sol_lamports: number
  token_base_units: number
}

export function useFaucet() {
  const config = useRuntimeConfig()
  const base = config.public.apiBase
  const { token } = useAuth()

  const status = ref<FaucetStatus | null>(null)
  const loading = ref(false)
  const claiming = ref(false)
  const failure = ref('')

  function headers(): Record<string, string> {
    return authHeaders()
  }

  async function load(): Promise<void> {
    if (!token.value) return
    loading.value = true
    failure.value = ''
    try {
      status.value = await $fetch<FaucetStatus>(`${base}/api/faucet`, { headers: headers() })
    } catch (error) {
      failure.value = apiErrorMessage(error, appT('start.faucet.error'))
    } finally {
      loading.value = false
    }
  }

  async function claim(): Promise<FaucetClaim | null> {
    if (claiming.value) return null
    claiming.value = true
    failure.value = ''
    try {
      const claimed = await $fetch<FaucetClaim>(`${base}/api/faucet`, {
        method: 'POST',
        headers: headers()
      })
      await load()
      return claimed
    } catch (error) {
      failure.value = apiErrorMessage(error, appT('start.faucet.error'))
      return null
    } finally {
      claiming.value = false
    }
  }

  return { status, loading, claiming, failure, load, claim }
}
