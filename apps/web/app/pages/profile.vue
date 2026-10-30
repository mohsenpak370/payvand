<script setup lang="ts">
import type { BudgetState } from '~/composables/useBudget'
import type { TransactionsResponse } from '~/composables/useAuth'

useSeoMeta({ title: 'Profile — Payvand' })
definePageMeta({ middleware: 'auth' })

const { user, walletChallenge, linkWallet, myTransactions } = useAuth()
const { publicKey, connected, connecting, canSignMessage } = useSolanaWallet()
const { execute: signMessage, loading: signing } = useSolanaSignMessage()
const budgetApi = useBudget()

const linkedWallet = computed(() => user.value?.wallet ?? null)

/** Linking: connect → ask the API for text → sign it → attach. */
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
    await refreshTransactions()
  } catch (error) {
    linkFailure.value = apiErrorMessage(
      error,
      'The wallet declined to sign, or the request expired. Try again.'
    )
  } finally {
    linking.value = false
  }
}

/** Monthly allowance, read from the connected wallet's budget account. */
const budget = ref<BudgetState | null>(null)
const budgetPot = ref(0)
const budgetLoading = ref(false)
const budgetError = ref('')

const spentThisMonth = computed(() => {
  const state = budget.value
  if (!state) return 0
  return state.monthIndex === currentMonthIndex() ? state.spent : 0
})

const remaining = computed(() =>
  budget.value
    ? Math.max(0, budget.value.monthlyLimit - spentThisMonth.value)
    : 0
)

const spentPercent = computed(() => {
  const limit = budget.value?.monthlyLimit ?? 0
  if (!limit) return 0
  return Math.min(100, Math.round((spentThisMonth.value / limit) * 100))
})

async function refreshBudget() {
  budget.value = null
  budgetPot.value = 0
  budgetError.value = ''
  if (!publicKey.value) return

  budgetLoading.value = true
  try {
    const loaded = await budgetApi.loadBudget(publicKey.value)
    budget.value = loaded
    if (loaded) budgetPot.value = await budgetApi.availablePot(loaded)
  } catch (error) {
    budgetError.value
      = error instanceof Error ? error.message : 'Could not read your allowance.'
  } finally {
    budgetLoading.value = false
  }
}

watch(publicKey, refreshBudget)

/** On-chain history for the linked wallet. */
const transactions = ref<TransactionsResponse | null>(null)
const txLoading = ref(false)
const txFailure = ref('')

async function refreshTransactions() {
  if (!linkedWallet.value) {
    transactions.value = null
    return
  }
  txFailure.value = ''
  txLoading.value = true
  try {
    transactions.value = await myTransactions()
  } catch (error) {
    txFailure.value = apiErrorMessage(error, 'Could not load your transactions.')
  } finally {
    txLoading.value = false
  }
}

function shorten(value: string): string {
  return value.length > 12 ? `${value.slice(0, 4)}…${value.slice(-4)}` : value
}

function formatDate(seconds: number): string {
  if (!seconds) return '—'
  return new Date(seconds * 1000).toLocaleString([], {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit'
  })
}

function statusColor(status: string): 'success' | 'warning' | 'neutral' {
  return status === 'settled'
    ? 'success'
    : status === 'pending' || status === 'missing'
      ? 'warning'
      : 'neutral'
}

onMounted(() => {
  refreshBudget()
  refreshTransactions()
})
</script>

<template>
  <UPageSection
    title="Profile"
    :description="`Signed in as ${user?.email ?? ''}`"
  >
    <div class="mx-auto flex max-w-3xl flex-col gap-6">
      <!-- Wallet: connect it, then link it to this account. -->
      <UCard>
        <template #header>
          <div class="flex items-center justify-between gap-3">
            <div>
              <h2 class="text-base font-semibold">
                Wallet
              </h2>
              <p class="text-sm text-muted">
                Link your Solana wallet to pay without typing a password and to see your transactions.
              </p>
            </div>
            <UBadge
              v-if="linkedWallet"
              color="success"
              variant="subtle"
            >
              Linked
            </UBadge>
          </div>
        </template>

        <div class="flex flex-col gap-4">
          <WalletConnect />

          <UAlert
            v-if="linkedWallet"
            color="success"
            variant="subtle"
            icon="i-lucide-wallet-cards"
            title="Linked to your account"
            :description="`This wallet signs you in: ${linkedWallet}`"
          />

          <UAlert
            v-else-if="connected && publicKey"
            color="info"
            variant="subtle"
            icon="i-lucide-link"
            title="Connect it to your account"
            description="One signature now, so you can sign in with this wallet later. It authorises no transaction."
          />

          <UAlert
            v-if="linkFailure"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            title="Could not link the wallet"
            :description="linkFailure"
          />

          <UButton
            v-if="connected && publicKey && !linkedWallet"
            :label="canSignMessage ? 'Link this wallet' : 'This wallet cannot sign messages'"
            icon="i-lucide-link"
            :loading="linking || signing || connecting"
            :disabled="!canSignMessage"
            @click="linkWalletNow"
          />
        </div>
      </UCard>

      <!-- Monthly allowance. -->
      <UCard>
        <template #header>
          <div>
            <h2 class="text-base font-semibold">
              Monthly allowance
            </h2>
            <p class="text-sm text-muted">
              Set a limit once — payments inside it need no wallet approval at all.
            </p>
          </div>
        </template>

        <div class="flex flex-col gap-4">
          <p
            v-if="!connected"
            class="text-sm text-muted"
          >
            Connect a wallet to see and set its allowance.
          </p>

          <UAlert
            v-else-if="budgetError"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            title="Could not read your allowance"
            :description="budgetError"
          />

          <p
            v-else-if="budgetLoading"
            class="py-4 text-center text-sm text-muted"
          >
            Reading your allowance…
          </p>

          <!-- The prompt: no allowance yet. -->
          <div
            v-else-if="!budget"
            class="flex flex-col items-start gap-3 rounded-xl border border-dashed border-default p-4"
          >
            <div class="flex items-center gap-2">
              <UIcon
                name="i-lucide-gauge"
                class="text-primary size-5"
              />
              <span class="font-medium">No allowance set yet</span>
            </div>
            <p class="text-sm text-muted">
              Pick a monthly budget in one wallet signature. Every payment inside
              it afterwards goes through with a single tap — no popups.
            </p>
            <UButton
              label="Set my monthly allowance"
              icon="i-lucide-arrow-right"
              to="/budget"
            />
          </div>

          <!-- The status: allowance exists. -->
          <template v-else>
            <div class="grid grid-cols-2 gap-4 md:grid-cols-3">
              <div class="flex flex-col gap-1">
                <span class="text-2xl font-semibold">{{ lamportsToSol(budget.monthlyLimit) }}</span>
                <span class="text-sm text-muted">Monthly limit (SOL)</span>
              </div>
              <div class="flex flex-col gap-1">
                <span class="text-2xl font-semibold">{{ lamportsToSol(spentThisMonth) }}</span>
                <span class="text-sm text-muted">Spent this month</span>
              </div>
              <div class="flex flex-col gap-1">
                <span class="text-2xl font-semibold text-success">{{ lamportsToSol(remaining) }}</span>
                <span class="text-sm text-muted">Remaining</span>
              </div>
            </div>

            <UProgress
              :value="spentPercent"
              :max="100"
              size="sm"
            />

            <div class="flex flex-wrap items-center justify-between gap-3 text-sm text-muted">
              <span>Pot available: {{ lamportsToSol(budgetPot) }} SOL</span>
              <UButton
                label="Manage allowance"
                color="neutral"
                variant="soft"
                size="sm"
                to="/budget"
              />
            </div>
          </template>
        </div>
      </UCard>

      <!-- Transactions. -->
      <UCard>
        <template #header>
          <div class="flex items-center justify-between gap-3">
            <div>
              <h2 class="text-base font-semibold">
                Transactions
              </h2>
              <p class="text-sm text-muted">
                {{
                  transactions
                    ? `${transactions.total} payment${transactions.total === 1 ? '' : 's'} on ${networkLabel(transactions.cluster)}`
                    : 'Everything your wallet has paid or been paid.'
                }}
              </p>
            </div>
            <UButton
              v-if="linkedWallet"
              icon="i-lucide-refresh-cw"
              label="Refresh"
              color="neutral"
              variant="soft"
              size="sm"
              :loading="txLoading"
              @click="refreshTransactions"
            />
          </div>
        </template>

        <div class="flex flex-col gap-3">
          <div
            v-if="!linkedWallet"
            class="flex flex-col items-start gap-3 rounded-xl border border-dashed border-default p-4"
          >
            <span class="font-medium">Link your wallet to see transactions</span>
            <p class="text-sm text-muted">
              Your payment history follows the wallet, so it appears here once the
              wallet is linked above.
            </p>
          </div>

          <UAlert
            v-else-if="txFailure"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            title="Could not load transactions"
            :description="txFailure"
          />

          <p
            v-else-if="txLoading && !transactions"
            class="py-4 text-center text-sm text-muted"
          >
            Loading transactions…
          </p>

          <p
            v-else-if="transactions && transactions.items.length === 0"
            class="py-4 text-center text-sm text-muted"
          >
            No transactions yet — your first payment will show up here.
          </p>

          <ul
            v-else-if="transactions"
            class="flex flex-col divide-y divide-default"
          >
            <li
              v-for="item in transactions.items"
              :key="item.id"
              class="flex items-center gap-3 py-2"
            >
              <UIcon
                :name="item.direction === 'received' ? 'i-lucide-arrow-down-left' : 'i-lucide-arrow-up-right'"
                :class="item.direction === 'received' ? 'text-success size-5' : 'text-error size-5'"
                class="shrink-0"
              />
              <div class="min-w-0 flex-1">
                <p class="truncate text-sm">
                  {{ item.memo || (item.direction === 'received' ? 'Payment received' : 'Payment sent') }}
                </p>
                <p class="truncate text-xs text-muted">
                  {{ formatDate(item.settled_at_unix || item.created_at_unix) }}
                  · {{ item.direction === 'received' ? 'from' : 'to' }}
                  {{ shorten(item.counterparty) }}
                </p>
              </div>
              <div class="flex shrink-0 flex-col items-end gap-1">
                <span class="text-sm font-medium">
                  {{ item.direction === 'received' ? '+' : '−' }}{{ lamportsToSol(item.amount_lamports) }} SOL
                </span>
                <UBadge
                  :color="statusColor(item.status)"
                  variant="subtle"
                  size="xs"
                >
                  {{ statusLabel(item.status) }}
                </UBadge>
              </div>
            </li>
          </ul>
        </div>
      </UCard>
    </div>
  </UPageSection>
</template>
