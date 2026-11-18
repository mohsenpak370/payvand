import type { WaitlistEntry, WaitlistPage } from './useAdminApi'

/**
 * The waiting list is its own read: it holds addresses, not accounts, and
 * loading it must not decide whether the rest of the page renders.
 */
export function useAdminWaitlist() {
  const { t } = useI18n()
  const { fetchWaitlist, removeWaitlistEntry } = useAdminApi()

  const waitlist = shallowRef<WaitlistPage | null>(null)
  const waitlistLoading = shallowRef(false)
  const waitlistFailure = shallowRef('')
  const waitlistBusy = shallowRef(false)

  async function loadWaitlist() {
    waitlistFailure.value = ''
    waitlistLoading.value = true
    try {
      waitlist.value = await fetchWaitlist()
    } catch (error) {
      waitlistFailure.value = apiErrorMessage(error, t('admin.waitlist.loadFailure'))
    } finally {
      waitlistLoading.value = false
    }
  }

  async function removeEntry(entry: WaitlistEntry) {
    const confirmed = window.confirm(t('admin.waitlist.removeConfirm', { email: entry.email }))
    if (!confirmed) return

    waitlistBusy.value = true
    try {
      await removeWaitlistEntry(entry.email)
      await loadWaitlist()
    } catch (error) {
      waitlistFailure.value = apiErrorMessage(error, t('admin.waitlist.removeFailure'))
    } finally {
      waitlistBusy.value = false
    }
  }

  onMounted(loadWaitlist)

  return {
    waitlist,
    waitlistLoading,
    waitlistFailure,
    waitlistBusy,
    loadWaitlist,
    removeEntry
  }
}
