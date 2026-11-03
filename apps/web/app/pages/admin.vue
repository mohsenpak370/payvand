<script setup lang="ts">
useSeoMeta({ title: 'Admin — Payvand' })
definePageMeta({ middleware: 'auth' })

const { fetchStats, fetchUsers, fetchPayments, deleteUser } = useAdminApi()

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
      failure.value = apiErrorMessage(error, 'Could not load the admin data.')
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
    feedFailure.value = apiErrorMessage(
      error,
      'Could not read recent payments from the cluster.'
    )
  } finally {
    feedLoading.value = false
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
  const confirmed = window.confirm(
    `Delete ${row.email}?\n\nThis removes the account, its sessions, its discount codes and any pending reset links. This cannot be undone.`
  )
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
    failure.value = apiErrorMessage(error, 'Could not delete that account.')
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

onMounted(() => {
  load()
  loadFeed()
})
</script>

<template>
  <UPageSection
    title="Admin"
    description="Platform oversight: accounts, live sessions and discount codes."
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
        title="No admin access"
        description="This account is not listed in the API's ADMIN_EMAILS setting, so the admin endpoints answer 403. Add the address there (and redeploy) to grant access."
      />

      <template v-else>
        <div class="grid grid-cols-2 gap-4 md:grid-cols-4">
          <UCard>
            <div class="flex flex-col gap-1">
              <span class="text-2xl font-semibold">{{ stats?.users ?? '—' }}</span>
              <span class="text-sm text-muted">Accounts</span>
            </div>
          </UCard>
          <UCard>
            <div class="flex flex-col gap-1">
              <span class="text-2xl font-semibold">{{ stats?.active_sessions ?? '—' }}</span>
              <span class="text-sm text-muted">Live sessions</span>
            </div>
          </UCard>
          <UCard>
            <div class="flex flex-col gap-1">
              <span class="text-2xl font-semibold">{{ stats?.discount_codes ?? '—' }}</span>
              <span class="text-sm text-muted">Discount codes</span>
            </div>
          </UCard>
          <UCard>
            <div class="flex flex-col gap-1">
              <span class="text-2xl font-semibold">{{ stats?.pending_resets ?? '—' }}</span>
              <span class="text-sm text-muted">Pending resets</span>
            </div>
          </UCard>
        </div>

        <UCard class="min-w-0">
          <template #header>
            <div class="flex items-center justify-between gap-3">
              <div>
                <h2 class="text-base font-semibold">
                  Recent payments
                </h2>
                <p class="text-xs text-muted">
                  {{
                    feed
                      ? `${feed.total} invoice${feed.total === 1 ? '' : 's'} on ${networkLabel(feed.cluster)}`
                      : 'Read live from the cluster'
                  }}
                </p>
              </div>
              <UButton
                icon="i-lucide-refresh-cw"
                label="Refresh"
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
            title="Cluster unavailable"
            :description="feedFailure"
          />
          <p
            v-else-if="feedLoading && !feed"
            class="py-6 text-center text-sm text-muted"
          >
            Reading the cluster…
          </p>
          <p
            v-else-if="feed && feed.items.length === 0"
            class="py-6 text-center text-sm text-muted"
          >
            No payments on chain yet.
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
                <tr class="border-b text-left text-muted">
                  <th class="py-2 pr-3 font-medium">
                    When
                  </th>
                  <th class="py-2 pr-3 font-medium">
                    Seller
                  </th>
                  <th class="py-2 pr-3 text-right font-medium">
                    Amount
                  </th>
                  <th class="py-2 pr-3 font-medium">
                    Memo
                  </th>
                  <th class="py-2 pr-3 font-medium">
                    Status
                  </th>
                  <th class="py-2 font-medium">
                    Paid by
                  </th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="payment in feed.items"
                  :key="payment.id"
                  class="border-b last:border-0"
                >
                  <td class="py-2 pr-3 whitespace-nowrap text-muted">
                    {{ formatDateTime(payment.settled_at_unix || payment.created_at_unix) }}
                  </td>
                  <td class="py-2 pr-3 font-mono text-xs">
                    {{ short(payment.seller) }}
                  </td>
                  <td class="py-2 pr-3 text-right">
                    {{ lamportsToSol(payment.amount_lamports) }}
                  </td>
                  <td class="max-w-[12rem] truncate py-2 pr-3">
                    {{ payment.memo || '—' }}
                  </td>
                  <td class="py-2 pr-3">
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
              placeholder="Search by email…"
              icon="i-lucide-search"
            />
            <UButton
              type="submit"
              label="Search"
              :loading="loading"
            />
          </form>

          <UAlert
            v-if="failure"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            title="Something went wrong"
            :description="failure"
          />

          <p
            v-else-if="loading"
            class="py-6 text-center text-sm text-muted"
          >
            Loading…
          </p>

          <p
            v-else-if="rows.length === 0"
            class="py-6 text-center text-sm text-muted"
          >
            No accounts match.
          </p>

          <div
            v-else
            class="overflow-x-auto"
          >
            <table class="w-full min-w-[32rem] text-sm">
              <thead>
                <tr class="border-b text-left text-muted">
                  <th class="py-2 pr-3 font-medium">
                    Account
                  </th>
                  <th class="py-2 pr-3 font-medium">
                    Signed up
                  </th>
                  <th class="py-2 pr-3 text-right font-medium">
                    Sessions
                  </th>
                  <th class="py-2 pr-3 text-right font-medium">
                    Codes
                  </th>
                  <th class="py-2 text-right font-medium">
                    Actions
                  </th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="row in rows"
                  :key="row.id"
                  class="border-b last:border-0"
                >
                  <td class="py-2 pr-3">
                    <div>{{ row.email }}</div>
                    <div
                      v-if="row.wallet"
                      class="text-muted text-xs"
                    >
                      {{ row.wallet }}
                    </div>
                  </td>
                  <td class="py-2 pr-3 whitespace-nowrap text-muted">
                    {{ formatDate(row.created_at) }}
                  </td>
                  <td class="py-2 pr-3 text-right">
                    {{ row.sessions }}
                  </td>
                  <td class="py-2 pr-3 text-right">
                    {{ row.codes }}
                  </td>
                  <td class="py-2 text-right">
                    <UButton
                      icon="i-lucide-trash-2"
                      color="error"
                      variant="subtle"
                      size="xs"
                      label="Delete"
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
            <span>{{ pageFrom }}–{{ pageTo }} of {{ total }} accounts</span>
            <div class="flex gap-2">
              <UButton
                label="Previous"
                color="neutral"
                variant="soft"
                size="sm"
                :disabled="offset === 0 || loading"
                @click="pageDown"
              />
              <UButton
                label="Next"
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
