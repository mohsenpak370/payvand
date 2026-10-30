<script setup lang="ts">
useSeoMeta({ title: 'Admin — Payvand' })
definePageMeta({ middleware: 'auth' })

const { fetchStats, fetchUsers, deleteUser } = useAdminApi()

const stats = ref<AdminStats | null>(null)
const rows = ref<AdminUser[]>([])
const search = ref('')
const loading = ref(true)
const busy = ref(false)
const forbidden = ref(false)
const failure = ref('')

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
    rows.value = await fetchUsers(search.value.trim() || undefined)
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

async function searchNow() {
  await load()
}

async function remove(row: AdminUser) {
  const confirmed = window.confirm(
    `Delete ${row.email}?\n\nThis removes the account, its sessions, its discount codes and any pending reset links. This cannot be undone.`
  )
  if (!confirmed) return

  busy.value = true
  try {
    await deleteUser(row.id)
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

onMounted(load)
</script>

<template>
  <UPageSection
    title="Admin"
    description="Platform oversight: accounts, live sessions and discount codes."
  >
    <div class="mx-auto flex max-w-4xl flex-col gap-6">
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

        <UCard>
          <form
            class="mb-4 flex gap-2"
            @submit.prevent="searchNow"
          >
            <UInput
              v-model="search"
              class="flex-1"
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

          <table
            v-else
            class="w-full text-sm"
          >
            <thead>
              <tr class="border-b text-left text-muted">
                <th class="py-2 font-medium">
                  Account
                </th>
                <th class="py-2 font-medium">
                  Signed up
                </th>
                <th class="py-2 text-right font-medium">
                  Sessions
                </th>
                <th class="py-2 text-right font-medium">
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
                <td class="py-2">
                  <div>{{ row.email }}</div>
                  <div
                    v-if="row.wallet"
                    class="text-xs text-muted"
                  >
                    {{ row.wallet }}
                  </div>
                </td>
                <td class="py-2 text-muted">
                  {{ formatDate(row.created_at) }}
                </td>
                <td class="py-2 text-right">
                  {{ row.sessions }}
                </td>
                <td class="py-2 text-right">
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
        </UCard>
      </template>
    </div>
  </UPageSection>
</template>
