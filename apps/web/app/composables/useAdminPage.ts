import { useAdminAirdrops } from './useAdminAirdrops'
import { useAdminChain } from './useAdminChain'
import { useAdminFeed } from './useAdminFeed'
import { useAdminRecords } from './useAdminRecords'
import { useAdminUsers } from './useAdminUsers'
import { useAdminWaitlist } from './useAdminWaitlist'

/** The dashboard's tabs, in the order they are shown. */
export const ADMIN_TABS = ['overview', 'accounts', 'records', 'payments', 'chain', 'airdrops'] as const
export type AdminTab = (typeof ADMIN_TABS)[number]

export function isAdminTab(value: unknown): value is AdminTab {
  return typeof value === 'string' && (ADMIN_TABS as readonly string[]).includes(value)
}

/**
 * Wires the dashboard's independent reads once.
 *
 * The API decides who is allowed in: a 403 on the stats read hides the page,
 * but every other read still loads and fails on its own. Each read owns its
 * state and actions; this coordinator only combines them, derives the one flag
 * that disables every delete button while any deletion runs, and loads the
 * cluster-backed reads the first time a tab that shows them is opened — so the
 * first paint never waits on an RPC the admin did not ask for.
 */
export function useAdminPage() {
  const users = useAdminUsers()
  const feed = useAdminFeed()
  const waitlist = useAdminWaitlist()
  const chain = useAdminChain()
  const airdrops = useAdminAirdrops()
  const records = useAdminRecords()

  const busy = computed(() => users.usersBusy.value || waitlist.waitlistBusy.value || records.recordsBusy.value)

  const opened = new Set<AdminTab>()
  /** Loads what a tab shows, once; its refresh buttons reload after that. */
  function openTab(tab: AdminTab) {
    if (opened.has(tab)) return
    opened.add(tab)
    if (tab === 'payments') void feed.loadFeed()
    if (tab === 'records') void records.loadRecords()
    if (tab === 'chain' || tab === 'airdrops') {
      // Both tabs show the server wallets; one read serves the two.
      if (!chain.chain.value && !chain.chainLoading.value) void chain.loadChain()
    }
    if (tab === 'airdrops') void airdrops.loadAirdrops()
  }

  return {
    ...users,
    ...feed,
    ...waitlist,
    ...chain,
    ...airdrops,
    ...records,
    busy,
    openTab
  }
}
