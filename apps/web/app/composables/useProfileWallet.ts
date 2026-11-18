import type { Ref } from 'vue'

interface UseProfileWalletOptions {
  /** The wallet the account currently signs in with. */
  linkedWallet: Readonly<Ref<string | null>>
  /** Re-read the history once a link or unlink changes whose payments they are. */
  onWalletChanged: () => Promise<void>
}

/**
 * Attaching and detaching the wallet that signs this account in. Linking asks
 * for one signature over a challenge; unlinking only needs the account's own
 * confirmation, because the email and password keep working either way.
 */
export function useProfileWallet({
  linkedWallet,
  onWalletChanged
}: UseProfileWalletOptions) {
  const { t } = useI18n()
  const route = useRoute()
  const { walletChallenge, linkWallet, unlinkWallet } = useAuth()
  const { publicKey, connected, connecting, canSignMessage } = useSolanaWallet()
  const { execute: signMessage, loading: signing } = useSolanaSignMessage()

  /** Fresh signups arrive as /profile?welcome=1 with this nudge to answer. */
  const showWelcome = computed(
    () => route.query.welcome === '1' && !linkedWallet.value
  )

  /** A connected wallet that is not the linked one can replace it. */
  const switchNeeded = computed(
    () =>
      connected.value
      && !!publicKey.value
      && !!linkedWallet.value
      && publicKey.value !== linkedWallet.value
  )

  const linkFailure = ref('')
  const linking = ref(false)

  async function linkWalletNow() {
    const address = publicKey.value
    if (!address) return

    linkFailure.value = ''
    linking.value = true
    try {
      const issued = await walletChallenge(address, 'link')
      const result = await signMessage(new TextEncoder().encode(issued.message))
      await linkWallet(address, base58Encode(result.signature))
      // The wallet now belongs to the account: the feed becomes available.
      await onWalletChanged()
    } catch (error) {
      linkFailure.value = apiErrorMessage(error, t('profile.linkFailure'))
    } finally {
      linking.value = false
    }
  }

  const unlinking = ref(false)
  const unlinkFailure = ref('')

  async function unlinkWalletNow() {
    const confirmed = window.confirm(t('profile.unlinkConfirm'))
    if (!confirmed) return

    unlinkFailure.value = ''
    unlinking.value = true
    try {
      await unlinkWallet()
      await onWalletChanged()
    } catch (error) {
      unlinkFailure.value = apiErrorMessage(error, t('profile.unlinkFailure'))
    } finally {
      unlinking.value = false
    }
  }

  return {
    publicKey,
    connected,
    connecting,
    canSignMessage,
    signing,
    linkedWallet,
    showWelcome,
    switchNeeded,
    linkFailure,
    linking,
    unlinkFailure,
    unlinking,
    linkWalletNow,
    unlinkWalletNow
  }
}
