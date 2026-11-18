import { useAdminFeed } from './useAdminFeed'
import { useAdminUsers } from './useAdminUsers'
import { useAdminWaitlist } from './useAdminWaitlist'

/**
 * Wires the dashboard's three independent reads once.
 *
 * The API decides who is allowed in: a 403 on the stats read hides the page,
 * but the feed and the waitlist still load and fail on their own. Each read
 * owns its state and actions; this coordinator only combines them and derives
 * the one flag that disables every delete button while any deletion runs.
 */
export function useAdminPage() {
  const users = useAdminUsers()
  const feed = useAdminFeed()
  const waitlist = useAdminWaitlist()

  const busy = computed(() => users.usersBusy.value || waitlist.waitlistBusy.value)

  return {
    ...users,
    ...feed,
    ...waitlist,
    busy
  }
}
