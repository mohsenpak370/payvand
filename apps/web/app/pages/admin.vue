<script setup lang="ts">
const { t } = useI18n()

useSeoMeta({ title: t('admin.metaTitle') })
definePageMeta({ middleware: 'auth' })

const {
  fetchStats,
  fetchUsers,
  fetchPayments,
  fetchWaitlist,
  removeWaitlistEntry,
  deleteUser
} = useAdminApi()

const stats = ref<AdminStats | null>(null)
const rows = ref<AdminUser[]>([])
const total = ref(0)
const pageSize = 20
const offset = ref(0)
const search = ref('')
const loading = ref(true)
const busy = ref(false)
const forbidden = ref(false)
const failure = ref('')

// The feed reads the chain, so it loads and fails apart from the SQL data:
// a flaky RPC must not blank the account table.
const feed = ref<PaymentsPage | null>(null)
const feedLoading = ref(false)
const feedFailure = ref('')

// The waiting list is its own read too: it holds addresses, not accounts, and
// loading it must not decide whether the rest of the page renders.
const waitlist = ref<WaitlistPage | null>(null)
const waitlistLoading = ref(false)
const waitlistFailure = ref('')

const pageFrom = computed(() => (total.value === 0 ? 0 : offset.value + 1))
const pageTo = computed(() => Math.min(offset.value + pageSize, total.value))

/** ofetch surfaces the HTTP status in one of several shapes. */
function statusOf(error: unknown): number | undefined {
  if (typeof error !== 'object' || error === null) return undefined
  const candidate = error as {
    status?: number
    statusCode?: number
    response?: { status?: number }
  }
  return candidate.status ?? candidate.statusCode ?? candidate.response?.status
}

async function load() {
  failure.value = ''
  loading.value = true
  try {
    stats.value = await fetchStats()
    const page = await fetchUsers(
      search.value.trim() || undefined,
      pageSize,
      offset.value
    )
    rows.value = page.items
    total.value = page.total
    forbidden.value = false
  } catch (error) {
    if (statusOf(error) === 403) {
      forbidden.value = true
    } else {
      failure.value = apiErrorMessage(error, t('admin.users.loadFailure'))
    }
  } finally {
    loading.value = false
  }
}

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

  busy.value = true
  try {
    await removeWaitlistEntry(entry.email)
    await loadWaitlist()
  } catch (error) {
    waitlistFailure.value = apiErrorMessage(error, t('admin.waitlist.removeFailure'))
  } finally {
    busy.value = false
  }
}

async function searchNow() {
  offset.value = 0
  await load()
}

function pageDown() {
  offset.value = Math.max(0, offset.value - pageSize)
  load()
}

function pageUp() {
  offset.value += pageSize
  load()
}

async function remove(row: AdminUser) {
  const confirmed = window.confirm(t('admin.users.deleteConfirm', { email: row.email }))
  if (!confirmed) return

  busy.value = true
  try {
    await deleteUser(row.id)
    // Stepping back a page keeps the table from going empty after the
    // last row on a later page is deleted.
    if (rows.value.length === 1 && offset.value > 0) {
      offset.value = Math.max(0, offset.value - pageSize)
    }
    await load()
  } catch (error) {
    failure.value = apiErrorMessage(error, t('admin.users.deleteFailure'))
  } finally {
    busy.value = false
  }
}

function formatDate(seconds: number): string {
  return new Date(seconds * 1000).toLocaleDateString()
}

function formatDateTime(seconds: number): string {
  return new Date(seconds * 1000).toLocaleString()
}

function short(address: string): string {
  return address.length > 12
    ? `${address.slice(0, 4)}…${address.slice(-4)}`
    : address
}

function statusColor(status: string): 'success' | 'warning' | 'neutral' {
  return status === 'settled'
    ? 'success'
    : status === 'pending' || status === 'missing'
      ? 'warning'
      : 'neutral'
}

/** "1 address" / "2 addresses", in whatever forms the locale defines. */
function addressCount(count: number) {
  return t('admin.waitlist.count', { count }, count)
}

/** "1 invoice" / "2 invoices", in whatever forms the locale defines. */
function invoiceCount(count: number) {
  return t('admin.feed.count', {
    count,
    cluster: networkLabel(feed.value?.cluster ?? '')
  }, count)
}

onMounted(() => {
  load()
  loadFeed()
  loadWaitlist()
})
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
        <div class="grid grid-cols-2 gap-4 md:grid-cols-4">
          <UCard>
            <div class="flex flex-col gap-1">
              <span class="text-2xl font-semibold">{{ stats?.users ?? '—' }}</span>
              <span class="text-sm text-muted">{{ $t('admin.stats.accounts') }}</span>
            </div>
          </UCard>
          <UCard>
            <div class="flex flex-col gap-1">
              <span class="text-2xl font-semibold">{{ stats?.active_sessions ?? '—' }}</span>
              <span class="text-sm text-muted">{{ $t('admin.stats.sessions') }}</span>
            </div>
          </UCard>
          <UCard>
            <div class="flex flex-col gap-1">
              <span class="text-2xl font-semibold">{{ stats?.discount_codes ?? '—' }}</span>
              <span class="text-sm text-muted">{{ $t('admin.stats.discounts') }}</span>
            </div>
          </UCard>
          <UCard>
            <div class="flex flex-col gap-1">
              <span class="text-2xl font-semibold">{{ stats?.pending_resets ?? '—' }}</span>
              <span class="text-sm text-muted">{{ $t('admin.stats.resets') }}</span>
            </div>
          </UCard>
        </div>

        <UCard class="min-w-0">
          <template #header>
            <div class="flex items-center justify-between gap-3">
              <div>
                <h2 class="text-base font-semibold">
                  {{ $t('admin.waitlist.title') }}
                </h2>
                <p class="text-xs text-muted">
                  {{
                    waitlist
                      ? addressCount(waitlist.total)
                      : $t('admin.waitlist.fallback')
                  }}
                </p>
              </div>
              <UButton
                icon="i-lucide-refresh-cw"
                :label="$t('profile.refresh')"
                color="neutral"
                variant="soft"
                size="sm"
                :loading="waitlistLoading"
                @click="loadWaitlist"
              />
            </div>
          </template>

          <UAlert
            v-if="waitlistFailure"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            :title="$t('admin.waitlist.errorTitle')"
            :description="waitlistFailure"
          />
          <p
            v-else-if="waitlistLoading && !waitlist"
            class="py-6 text-center text-sm text-muted"
          >
            {{ $t('admin.waitlist.reading') }}
          </p>
          <p
            v-else-if="!waitlist?.items.length"
            class="py-6 text-center text-sm text-muted"
          >
            {{ $t('admin.waitlist.empty') }}
          </p>
          <ul
            v-else
            class="flex flex-col divide-y divide-default"
          >
            <li
              v-for="entry in waitlist.items"
              :key="entry.email"
              class="flex min-w-0 items-center justify-between gap-3 py-2.5"
            >
              <div class="min-w-0">
                <p class="text-sm break-all">
                  {{ entry.email }}
                </p>
                <p class="text-xs text-muted">
                  {{ $t('admin.waitlist.joined', { date: formatDate(entry.created_at_unix) }) }}
                </p>
              </div>
              <UButton
                icon="i-lucide-trash-2"
                color="error"
                variant="ghost"
                size="sm"
                :disabled="busy"
                :aria-label="$t('admin.waitlist.removeAria')"
                @click="removeEntry(entry)"
              />
            </li>
          </ul>
        </UCard>

        <UCard class="min-w-0">
          <template #header>
            <div class="flex items-center justify-between gap-3">
              <div>
                <h2 class="text-base font-semibold">
                  {{ $t('admin.feed.title') }}
                </h2>
                <p class="text-xs text-muted">
                  {{
                    feed
                      ? invoiceCount(feed.total)
                      : $t('admin.feed.fallback')
                  }}
                </p>
              </div>
              <UButton
                icon="i-lucide-refresh-cw"
                :label="$t('profile.refresh')"
                color="neutral"
                variant="soft"
                size="sm"
                :loading="feedLoading"
                @click="loadFeed"
              />
            </div>
          </template>

          <UAlert
            v-if="feedFailure"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            :title="$t('admin.feed.errorTitle')"
            :description="feedFailure"
          />
          <p
            v-else-if="feedLoading && !feed"
            class="py-6 text-center text-sm text-muted"
          >
            {{ $t('admin.feed.reading') }}
          </p>
          <p
            v-else-if="feed && feed.items.length === 0"
            class="py-6 text-center text-sm text-muted"
          >
            {{ $t('admin.feed.empty') }}
          </p>
          <!--
            Six columns cannot fit a phone, so the table keeps a readable
            width and scrolls sideways inside the card instead of stretching
            the page. `overflow-x-auto` also stops the table's minimum width
            from forcing every ancestor wider than the viewport.
          -->
          <div
            v-else-if="feed"
            class="overflow-x-auto"
          >
            <table class="w-full min-w-[44rem] text-sm">
              <thead>
                <tr class="border-b text-start text-muted">
                  <th class="py-2 pe-3 font-medium">
                    {{ $t('admin.feed.when') }}
                  </th>
                  <th class="py-2 pe-3 font-medium">
                    {{ $t('admin.feed.seller') }}
                  </th>
                  <th class="py-2 pe-3 text-right font-medium">
                    {{ $t('admin.feed.amount') }}
                  </th>
                  <th class="py-2 pe-3 font-medium">
                    {{ $t('admin.feed.memo') }}
                  </th>
                  <th class="py-2 pe-3 font-medium">
                    {{ $t('admin.feed.status') }}
                  </th>
                  <th class="py-2 font-medium">
                    {{ $t('admin.feed.paidBy') }}
                  </th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="payment in feed.items"
                  :key="payment.id"
                  class="border-b last:border-0"
                >
                  <td class="py-2 pe-3 whitespace-nowrap text-muted">
                    {{ formatDateTime(payment.settled_at_unix || payment.created_at_unix) }}
                  </td>
                  <td class="py-2 pe-3 font-mono text-xs">
                    {{ short(payment.seller) }}
                  </td>
                  <td class="py-2 pe-3 text-right">
                    {{ lamportsToSol(payment.amount_lamports) }}
                  </td>
                  <td class="max-w-[12rem] truncate py-2 pe-3">
                    {{ payment.memo || '—' }}
                  </td>
                  <td class="py-2 pe-3">
                    <UBadge
                      :color="statusColor(payment.status)"
                      variant="subtle"
                    >
                      {{ statusLabel(payment.status) }}
                    </UBadge>
                  </td>
                  <td class="py-2 font-mono text-xs">
                    {{ payment.settled_by ? short(payment.settled_by) : '—' }}
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </UCard>

        <UCard class="min-w-0">
          <form
            class="mb-4 flex gap-2"
            @submit.prevent="searchNow"
          >
            <UInput
              v-model="search"
              class="min-w-0 flex-1"
              :placeholder="$t('admin.users.searchPlaceholder')"
              icon="i-lucide-search"
            />
            <UButton
              type="submit"
              :label="$t('admin.users.search')"
              :loading="loading"
            />
          </form>

          <UAlert
            v-if="failure"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            :title="$t('admin.users.errorTitle')"
            :description="failure"
          />

          <p
            v-else-if="loading"
            class="py-6 text-center text-sm text-muted"
          >
            {{ $t('common.loading') }}
          </p>

          <p
            v-else-if="rows.length === 0"
            class="py-6 text-center text-sm text-muted"
          >
            {{ $t('admin.users.empty') }}
          </p>

          <div
            v-else
            class="overflow-x-auto"
          >
            <table class="w-full min-w-[32rem] text-sm">
              <thead>
                <tr class="border-b text-start text-muted">
                  <th class="py-2 pe-3 font-medium">
                    {{ $t('admin.users.account') }}
                  </th>
                  <th class="py-2 pe-3 font-medium">
                    {{ $t('admin.users.signedUp') }}
                  </th>
                  <th class="py-2 pe-3 text-right font-medium">
                    {{ $t('admin.users.sessions') }}
                  </th>
                  <th class="py-2 pe-3 text-right font-medium">
                    {{ $t('admin.users.codes') }}
                  </th>
                  <th class="py-2 text-right font-medium">
                    {{ $t('admin.users.actions') }}
                  </th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="row in rows"
                  :key="row.id"
                  class="border-b last:border-0"
                >
                  <td class="py-2 pe-3">
                    <div>{{ row.email }}</div>
                    <div
                      v-if="row.wallet"
                      class="text-muted text-xs"
                    >
                      {{ row.wallet }}
                    </div>
                  </td>
                  <td class="py-2 pe-3 whitespace-nowrap text-muted">
                    {{ formatDate(row.created_at) }}
                  </td>
                  <td class="py-2 pe-3 text-right">
                    {{ row.sessions }}
                  </td>
                  <td class="py-2 pe-3 text-right">
                    {{ row.codes }}
                  </td>
                  <td class="py-2 text-right">
                    <UButton
                      icon="i-lucide-trash-2"
                      color="error"
                      variant="subtle"
                      size="xs"
                      :label="$t('admin.users.delete')"
                      :loading="busy"
                      @click="remove(row)"
                    />
                  </td>
                </tr>
              </tbody>
            </table>
          </div>

          <div
            v-if="total > pageSize"
            class="mt-4 flex flex-wrap items-center justify-between gap-3 text-sm text-muted"
          >
            <span>{{ $t('admin.users.range', { from: pageFrom, to: pageTo, total }) }}</span>
            <div class="flex gap-2">
              <UButton
                :label="$t('admin.users.previous')"
                color="neutral"
                variant="soft"
                size="sm"
                :disabled="offset === 0 || loading"
                @click="pageDown"
              />
              <UButton
                :label="$t('admin.users.next')"
                color="neutral"
                variant="soft"
                size="sm"
                :disabled="pageTo >= total || loading"
                @click="pageUp"
              />
            </div>
          </div>
        </UCard>
      </template>
    </div>
  </UPageSection>
</template>
