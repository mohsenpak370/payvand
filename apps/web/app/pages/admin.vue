<script setup lang="ts">
import type { TabsItem } from '@nuxt/ui'
import { ADMIN_TABS, isAdminTab, type AdminTab } from '~/composables/useAdminPage'
import type { RecordEdit } from '~/composables/useAdminRecords'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()

useSeoMeta({ title: t('admin.metaTitle') })
definePageMeta({ middleware: 'auth' })

const {
  stats,
  rows,
  total,
  pageSize,
  offset,
  usersLoading,
  usersFailure,
  forbidden,
  searchUsers,
  pageDown,
  pageUp,
  removeUser,
  feed,
  feedLoading,
  feedFailure,
  loadFeed,
  waitlist,
  waitlistLoading,
  waitlistFailure,
  loadWaitlist,
  removeEntry,
  chain,
  chainLoading,
  chainFailure,
  loadChain,
  airdrops,
  airdropsLoading,
  airdropsFailure,
  loadAirdrops,
  airdropsPrevious,
  airdropsNext,
  recordKind,
  shops,
  products,
  codes,
  recordSearches,
  recordsLoading,
  recordsFailure,
  recordsBusy,
  showRecords,
  searchRecords,
  recordsPrevious,
  recordsNext,
  removeRecord,
  saveRecord,
  busy,
  openTab
} = useAdminPage()

const ICONS: Record<AdminTab, string> = {
  overview: 'i-lucide-layout-dashboard',
  accounts: 'i-lucide-users',
  records: 'i-lucide-database',
  payments: 'i-lucide-receipt',
  chain: 'i-lucide-blocks',
  airdrops: 'i-lucide-droplets'
}

const tabs = computed<TabsItem[]>(() =>
  ADMIN_TABS.map(value => ({ value, label: t(`admin.tabs.${value}`), icon: ICONS[value] }))
)

/**
 * The open tab lives in `?tab=`, so a reload, a shared link or the back button
 * lands on the same view. An unknown value falls back to the overview.
 */
const active = computed<AdminTab>({
  get: () => (isAdminTab(route.query.tab) ? route.query.tab : 'overview'),
  set: (tab) => {
    void router.replace({ query: { ...route.query, tab: tab === 'overview' ? undefined : tab } })
  }
})

watch(active, openTab, { immediate: true })

/** The record the edit dialog has open, if any. */
const editing = shallowRef<RecordEdit | null>(null)

async function saveEdit(edit: RecordEdit, body: Record<string, unknown>, done: (failure: string) => void) {
  done(await saveRecord(edit, body))
}
</script>

<template>
  <UPageSection
    :title="$t('admin.title')"
    :description="$t('admin.description')"
  >
    <!--
      `min-w-0` on this column and on the cards below: in a flex column each
      item defaults to `min-width: auto`, so a table's minimum width is
      otherwise pushed up through every card and widens the page instead of
      scrolling inside it.
    -->
    <div class="mx-auto flex w-full max-w-4xl min-w-0 flex-col gap-6">
      <UAlert
        v-if="forbidden"
        color="error"
        variant="subtle"
        icon="i-lucide-shield-x"
        :title="$t('admin.forbiddenTitle')"
        :description="$t('admin.forbiddenDescription')"
      />

      <template v-else>
        <!-- The tab bar scrolls sideways on a phone rather than wrapping. -->
        <div class="overflow-x-auto">
          <UTabs
            v-model="active"
            :items="tabs"
            :content="false"
            variant="link"
            class="min-w-max"
          />
        </div>

        <!--
          `v-show`, not `v-if`: switching tabs keeps each card's search draft
          and page, and never re-reads what was already loaded.
        -->
        <div
          v-show="active === 'overview'"
          class="flex min-w-0 flex-col gap-6"
        >
          <AdminStatsCards :stats="stats" />

          <AdminWaitlistCard
            :waitlist="waitlist"
            :loading="waitlistLoading"
            :failure="waitlistFailure"
            :busy="busy"
            @refresh="loadWaitlist"
            @remove="removeEntry"
          />
        </div>

        <div
          v-show="active === 'accounts'"
          class="flex min-w-0 flex-col gap-6"
        >
          <AdminUsersCard
            :rows="rows"
            :total="total"
            :page-size="pageSize"
            :offset="offset"
            :loading="usersLoading"
            :failure="usersFailure"
            :busy="busy"
            @search="searchUsers"
            @delete="removeUser"
            @previous="pageDown"
            @next="pageUp"
          />
        </div>

        <div
          v-show="active === 'records'"
          class="flex min-w-0 flex-col gap-6"
        >
          <AdminRecordsCard
            :kind="recordKind"
            :shops="shops"
            :products="products"
            :codes="codes"
            :searches="recordSearches"
            :loading="recordsLoading"
            :failure="recordsFailure"
            :busy="busy"
            @show="showRecords"
            @search="searchRecords"
            @previous="recordsPrevious"
            @next="recordsNext"
            @edit="editing = $event"
            @remove="removeRecord"
          />
          <AdminRecordEditModal
            :edit="editing"
            :busy="recordsBusy"
            @close="editing = null"
            @save="saveEdit"
          />
        </div>

        <div
          v-show="active === 'payments'"
          class="flex min-w-0 flex-col gap-6"
        >
          <AdminPaymentsCard
            :feed="feed"
            :loading="feedLoading"
            :failure="feedFailure"
            @refresh="loadFeed"
          />
        </div>

        <div
          v-show="active === 'chain'"
          class="flex min-w-0 flex-col gap-6"
        >
          <AdminChainCard
            :chain="chain"
            :loading="chainLoading"
            :failure="chainFailure"
            @refresh="loadChain"
          />
          <AdminWalletsCard
            :chain="chain"
            :loading="chainLoading"
            :failure="chainFailure"
            @refresh="loadChain"
          />
        </div>

        <div
          v-show="active === 'airdrops'"
          class="flex min-w-0 flex-col gap-6"
        >
          <AdminWalletsCard
            :chain="chain"
            :loading="chainLoading"
            :failure="chainFailure"
            :only="['faucet']"
            @refresh="loadChain"
          />
          <AdminAirdropsCard
            :airdrops="airdrops"
            :loading="airdropsLoading"
            :failure="airdropsFailure"
            @refresh="loadAirdrops"
            @previous="airdropsPrevious"
            @next="airdropsNext"
          />
        </div>
      </template>
    </div>
  </UPageSection>
</template>
