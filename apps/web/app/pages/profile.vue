<script setup lang="ts">
import type { BudgetState } from '~/composables/useBudget'
import type { TransactionsResponse } from '~/composables/useAuth'
import type { PaymentQrPayload, SavedPayment } from '~/composables/usePaymentApi'
import type { Address } from '@vue-solana/nuxt/kit'

const { t } = useI18n()

useSeoMeta({ title: t('profile.metaTitle') })
definePageMeta({ middleware: 'auth' })

const route = useRoute()
const { user, walletChallenge, linkWallet, unlinkWallet, myTransactions } = useAuth()
const { publicKey, connected, connecting, canSignMessage } = useSolanaWallet()
const { execute: signMessage, loading: signing } = useSolanaSignMessage()
const budgetApi = useBudget()
const paymentApi = usePaymentApi()

const linkedWallet = computed(() => user.value?.wallet ?? null)

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
    linkFailure.value = apiErrorMessage(error, t('profile.linkFailure'))
  } finally {
    linking.value = false
  }
}

/** Detach the wallet — email and password keep working. */
const unlinking = ref(false)
const unlinkFailure = ref('')

async function unlinkWalletNow() {
  const confirmed = window.confirm(t('profile.unlinkConfirm'))
  if (!confirmed) return

  unlinkFailure.value = ''
  unlinking.value = true
  try {
    await unlinkWallet()
    await refreshTransactions()
  } catch (error) {
    unlinkFailure.value = apiErrorMessage(error, t('profile.unlinkFailure'))
  } finally {
    unlinking.value = false
  }
}

/** Monthly allowance, read from the linked (or connected) wallet's budget account. */
const budget = ref<BudgetState | null>(null)
const budgetPot = ref(0)
const budgetLoading = ref(false)
const budgetError = ref('')

/**
 * The allowance belongs to the account's linked wallet — public on-chain
 * state that needs no browser connection. Only fall back to the transient
 * browser wallet when nothing is linked yet.
 */
const allowanceAddress = computed<Address | null>(
  // The API stores the linked wallet as a validated base58 address.
  () => (linkedWallet.value as Address | null) ?? publicKey.value
)

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
  const address = allowanceAddress.value
  if (!address) return

  budgetLoading.value = true
  try {
    const loaded = await budgetApi.loadBudget(address)
    budget.value = loaded
    if (loaded) budgetPot.value = await budgetApi.availablePot(loaded)
  } catch (error) {
    budgetError.value
      = error instanceof Error ? error.message : t('profile.allowanceError')
  } finally {
    budgetLoading.value = false
  }
}

watch(allowanceAddress, refreshBudget, { immediate: true })

/** On-chain history for the linked wallet. */
const transactions = ref<TransactionsResponse | null>(null)
const txLoading = ref(false)
const txFailure = ref('')

// Both lists run long. The saved codes start folded, so someone who came for
// the wallet or the allowance is not scrolled past them — but the payments are
// the record of what happened and who it happened with, so they start open.
const savedOpen = ref(false)
const txOpen = ref(true)

// Payment codes this account saved. The code is the invoice, so everything
// shown on a card comes out of the payload the API kept.
const savedPayments = ref<SavedPayment[]>([])
const savedLoading = ref(false)
const savedFailure = ref('')

const savedCards = computed(() => savedPayments.value.flatMap((item) => {
  try {
    return [{ ...item, code: parseQrPayload(item.payload), link: paymentUrl(item.payload) }]
  } catch {
    // A code that no longer parses is not worth showing.
    return []
  }
}))

/** What the buyer pays: a saved code may carry its own discount. */
const cardPrice = (code: PaymentQrPayload) =>
  code.discount
    ? discountedLamports(code.amount_lamports, code.discount.percent_off)
    : code.amount_lamports

async function refreshSavedPayments() {
  savedFailure.value = ''
  savedLoading.value = true
  try {
    savedPayments.value = await paymentApi.listSavedPayments()
  } catch (error) {
    savedFailure.value = apiErrorMessage(error, t('profile.savedFailure'))
  } finally {
    savedLoading.value = false
  }
}

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
    txFailure.value = apiErrorMessage(error, t('profile.txFailure'))
  } finally {
    txLoading.value = false
  }
}

function shorten(value: string): string {
  return value.length > 12 ? `${value.slice(0, 4)}…${value.slice(-4)}` : value
}

/**
 * Finding one payment. A seller who did not create the code — the payment was
 * read out from the customer's screen — has only the reference code and the
 * day to go on, so those are the two ways in.
 */
const txQuery = ref('')
const txFrom = ref('')
const txTo = ref('')

/** The day input as unix seconds, at the start or the end of that day. */
function dayBound(date: string, end: boolean): number | null {
  if (!date) return null
  const at = new Date(`${date}T00:00:00`)
  if (Number.isNaN(at.getTime())) return null
  return Math.floor(at.getTime() / 1000) + (end ? 86_399 : 0)
}

const txFiltering = computed(() => Boolean(txQuery.value.trim() || txFrom.value || txTo.value))

const filteredTransactions = computed(() => {
  const items = transactions.value?.items ?? []
  const query = txQuery.value.trim().toUpperCase()
  const from = dayBound(txFrom.value, false)
  const to = dayBound(txTo.value, true)
  return items.filter((item) => {
    // A code is matched on the whole string, not by prefix: a customer reading
    // eight characters out does not guarantee they are the first eight. A
    // longer paste is a payment address, so it is matched as one.
    if (query && !referenceCode(item.id).includes(query) && !item.id.toUpperCase().startsWith(query)) {
      return false
    }
    // An invoice that is still unpaid has no settle time; the day it was
    // created is the only date it has.
    const at = item.settled_at_unix || item.created_at_unix
    if (from !== null && at < from) return false
    if (to !== null && at > to) return false
    return true
  })
})

function clearTxFilters() {
  txQuery.value = ''
  txFrom.value = ''
  txTo.value = ''
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

// Saved codes live in a shop, so the card that lists them belongs to accounts
// that use shops at all — not to everyone who opens their profile.
const { shopsEnabled } = useShopPreference()

/** "1 payment" / "2 payments", in whatever forms the locale defines. */
function paymentCount(count: number, shown?: number) {
  return shown === undefined
    ? t('profile.txCount', { count, cluster: networkLabel(transactions.value?.cluster ?? '') }, count)
    : t('profile.txCountFiltered', {
        shown,
        total: count,
        cluster: networkLabel(transactions.value?.cluster ?? '')
      }, count)
}

onMounted(() => {
  refreshTransactions()
  if (shopsEnabled.value) refreshSavedPayments()
})
</script>

<template>
  <UPageSection
    :title="$t('profile.title')"
    :description="$t('profile.signedInAs', { email: user?.email ?? '' })"
    :ui="{ description: 'break-words' }"
  >
    <!-- `min-w-0` here and on the cards: in a flex column each item defaults to
         `min-width: auto`, so a long unbreakable token (wallet address, memo)
         would widen the whole page instead of wrapping or truncating. -->
    <div class="mx-auto flex w-full max-w-3xl min-w-0 flex-col gap-6">
      <!-- Currency and the shops switch moved here out of this page. The
           button keeps them reachable without hunting through the menu. -->
      <div class="flex justify-end">
        <UButton
          to="/settings"
          :label="$t('profile.settings')"
          icon="i-lucide-settings"
          color="neutral"
          variant="ghost"
          size="sm"
        />
      </div>

      <!-- Wallet: connect it, then link it to this account. -->
      <UCard class="min-w-0">
        <template #header>
          <div class="flex items-center justify-between gap-3">
            <div>
              <h2 class="text-base font-semibold">
                {{ $t('profile.walletTitle') }}
              </h2>
              <p class="text-sm text-muted">
                {{ $t('profile.walletDescription') }}
              </p>
            </div>
            <UBadge
              v-if="linkedWallet"
              color="success"
              variant="subtle"
            >
              {{ $t('profile.linked') }}
            </UBadge>
          </div>
        </template>

        <div class="flex flex-col gap-4">
          <UAlert
            v-if="showWelcome"
            color="info"
            variant="subtle"
            icon="i-lucide-party-popper"
            :title="$t('profile.welcomeTitle')"
            :description="$t('profile.welcomeDescription')"
          />

          <WalletConnect />

          <UAlert
            v-if="linkedWallet"
            color="success"
            variant="subtle"
            icon="i-lucide-wallet-cards"
            :title="$t('profile.linkedTitle')"
          >
            <!--
              The address has no spaces to break at, so it is allowed to break
              mid-token; otherwise it pushes the whole card wider than a phone.
            -->
            <template #description>
              {{ $t('profile.linkedSignsIn') }}
              <span class="break-all">{{ linkedWallet }}</span>
            </template>
          </UAlert>

          <UAlert
            v-else-if="connected && publicKey"
            color="info"
            variant="subtle"
            icon="i-lucide-link"
            :title="$t('profile.connectTitle')"
            :description="$t('profile.connectDescription')"
          />

          <UAlert
            v-if="linkedWallet && switchNeeded && publicKey"
            color="warning"
            variant="subtle"
            icon="i-lucide-repeat"
            :title="$t('profile.differentWalletTitle')"
            :description="$t('profile.differentWalletDescription', {
              linked: shorten(linkedWallet),
              connected: shorten(publicKey)
            })"
          />

          <UAlert
            v-if="linkFailure"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            :title="$t('profile.linkFailureTitle')"
            :description="linkFailure"
          />

          <UAlert
            v-if="unlinkFailure"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            :title="$t('profile.unlinkFailureTitle')"
            :description="unlinkFailure"
          />

          <UButton
            v-if="connected && publicKey && !linkedWallet"
            :label="canSignMessage ? $t('profile.linkThisWallet') : $t('profile.cannotSign')"
            icon="i-lucide-link"
            :loading="linking || signing || connecting"
            :disabled="!canSignMessage"
            @click="linkWalletNow"
          />

          <UButton
            v-if="switchNeeded && publicKey && canSignMessage"
            :label="$t('profile.switchWallet')"
            icon="i-lucide-repeat"
            :loading="linking || signing"
            @click="linkWalletNow"
          />

          <div
            v-if="linkedWallet"
            class="flex flex-wrap items-center justify-between gap-3 border-t border-default pt-3"
          >
            <p class="text-xs text-muted">
              {{ $t('profile.unlinkHint') }}
            </p>
            <UButton
              :label="$t('profile.unlink')"
              icon="i-lucide-unlink"
              color="neutral"
              variant="soft"
              size="sm"
              :loading="unlinking"
              @click="unlinkWalletNow"
            />
          </div>
        </div>
      </UCard>

      <!-- Monthly allowance. -->
      <UCard class="min-w-0">
        <template #header>
          <div>
            <h2 class="text-base font-semibold">
              {{ $t('profile.allowanceTitle') }}
            </h2>
            <p class="text-sm text-muted">
              {{ $t('profile.allowanceDescription') }}
            </p>
          </div>
        </template>

        <div class="flex flex-col gap-4">
          <p
            v-if="!allowanceAddress"
            class="text-sm text-muted"
          >
            {{ $t('profile.connectForAllowance') }}
          </p>

          <UAlert
            v-else-if="budgetError"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            :title="$t('profile.allowanceErrorTitle')"
            :description="budgetError"
          />

          <p
            v-else-if="budgetLoading"
            class="py-4 text-center text-sm text-muted"
          >
            {{ $t('profile.allowanceLoading') }}
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
              <span class="font-medium">{{ $t('profile.noAllowance') }}</span>
            </div>
            <p class="text-sm text-muted">
              {{ $t('profile.noAllowanceBody') }}
            </p>
            <UButton
              :label="$t('profile.setAllowance')"
              icon="i-lucide-arrow-right"
              to="/budget"
            />
          </div>

          <!-- The status: allowance exists. -->
          <template v-else>
            <div class="grid grid-cols-2 gap-3 sm:grid-cols-3 sm:gap-4">
              <div class="flex min-w-0 flex-col gap-1">
                <span class="text-xl font-semibold sm:text-2xl">{{ lamportsToSol(budget.monthlyLimit) }}</span>
                <span class="text-sm text-muted">{{ $t('profile.monthlyLimit') }}</span>
              </div>
              <div class="flex min-w-0 flex-col gap-1">
                <span class="text-xl font-semibold sm:text-2xl">{{ lamportsToSol(spentThisMonth) }}</span>
                <span class="text-sm text-muted">{{ $t('profile.spentThisMonth') }}</span>
              </div>
              <div class="flex min-w-0 flex-col gap-1">
                <span class="text-success text-xl font-semibold sm:text-2xl">{{ lamportsToSol(remaining) }}</span>
                <span class="text-sm text-muted">{{ $t('profile.remaining') }}</span>
              </div>
            </div>

            <!-- Usage meter, labelled so it never reads as a loading spinner. -->
            <div
              v-if="spentPercent > 0"
              class="flex flex-col gap-1.5"
            >
              <div class="flex items-center justify-between text-sm">
                <span class="text-muted">{{ $t('profile.usedThisMonth') }}</span>
                <span class="font-medium">{{ spentPercent }}%</span>
              </div>
              <UProgress
                :value="spentPercent"
                :max="100"
                size="sm"
              />
            </div>

            <div class="flex flex-wrap items-center justify-between gap-3 text-sm text-muted">
              <span>{{ $t('profile.potAvailable', { amount: lamportsToSol(budgetPot) }) }}</span>
              <UButton
                :label="$t('profile.manageAllowance')"
                color="neutral"
                variant="soft"
                size="sm"
                to="/budget"
              />
            </div>
          </template>
        </div>
      </UCard>

      <!-- Saved payment codes: a shop feature, so it shows for shop accounts. -->
      <UCard
        v-if="shopsEnabled"
        class="min-w-0"
      >
        <template #header>
          <div class="flex items-center justify-between gap-3">
            <div>
              <h2 class="text-base font-semibold">
                {{ $t('profile.savedTitle') }}
              </h2>
              <p class="text-sm text-muted">
                {{ $t('profile.savedDescription') }}
              </p>
            </div>
            <UButton
              icon="i-lucide-refresh-cw"
              :label="$t('profile.refresh')"
              color="neutral"
              variant="soft"
              size="sm"
              :loading="savedLoading"
              @click="refreshSavedPayments"
            />
          </div>
        </template>

        <div class="flex flex-col gap-3">
          <UAlert
            v-if="savedFailure"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            :title="$t('profile.savedErrorTitle')"
            :description="savedFailure"
          />

          <p
            v-else-if="savedLoading && !savedPayments.length"
            class="py-4 text-center text-sm text-muted"
          >
            {{ $t('profile.savedLoading') }}
          </p>

          <p
            v-else-if="!savedPayments.length"
            class="py-4 text-center text-sm text-muted"
          >
            {{ $t('profile.savedEmpty') }}
          </p>

          <UButton
            v-else-if="!savedOpen"
            :label="$t('profile.showSaved', { count: savedCards.length }, savedCards.length)"
            icon="i-lucide-chevron-down"
            color="neutral"
            variant="ghost"
            block
            @click="savedOpen = true"
          />

          <ul
            v-else
            class="grid gap-3 sm:grid-cols-2"
          >
            <li
              v-for="card in savedCards"
              :key="card.id"
              class="flex min-w-0 items-start gap-3 rounded-xl border border-default p-3"
            >
              <PaymentQr
                :payload="card.payload"
                :size="96"
                class="shrink-0"
              />
              <div class="flex min-w-0 flex-col gap-0.5">
                <p class="text-sm font-medium break-words">
                  {{ card.code.name || $t('profile.paymentFallback') }}
                </p>
                <p class="text-sm font-semibold">
                  {{ lamportsToSol(cardPrice(card.code)) }} SOL
                  <span
                    v-if="card.code.discount"
                    class="ms-1 text-xs font-normal text-muted line-through"
                  >{{ lamportsToSol(card.code.amount_lamports) }} SOL</span>
                </p>
                <p
                  v-if="card.code.description || card.code.memo"
                  class="text-xs text-muted break-words"
                >
                  {{ card.code.description || card.code.memo }}
                </p>
                <p class="text-xs text-muted break-words">
                  {{ networkLabel(card.code.cluster) }} · {{ formatDate(card.created_at_unix) }}
                </p>
                <UButton
                  :to="card.link"
                  target="_blank"
                  rel="noopener"
                  :label="$t('profile.openPayment')"
                  icon="i-lucide-external-link"
                  color="neutral"
                  variant="soft"
                  size="xs"
                  class="mt-1 self-start"
                />
              </div>
            </li>
          </ul>

          <UButton
            v-if="savedOpen"
            :label="$t('profile.hide')"
            icon="i-lucide-chevron-up"
            color="neutral"
            variant="ghost"
            block
            @click="savedOpen = false"
          />
        </div>
      </UCard>

      <!-- Transactions. -->
      <UCard class="min-w-0">
        <template #header>
          <div class="flex items-center justify-between gap-3">
            <div>
              <h2 class="text-base font-semibold">
                {{ $t('profile.txTitle') }}
              </h2>
              <p class="text-sm text-muted">
                {{
                  transactions
                    ? paymentCount(transactions.total, txFiltering ? filteredTransactions.length : undefined)
                    : $t('profile.txDescription')
                }}
              </p>
            </div>
            <UButton
              v-if="linkedWallet"
              icon="i-lucide-refresh-cw"
              :label="$t('profile.refresh')"
              color="neutral"
              variant="soft"
              size="sm"
              :loading="txLoading"
              @click="refreshTransactions"
            />
          </div>
        </template>

        <div class="flex flex-col gap-3">
          <!--
            Finding one payment: the code the customer read out, or the days it
            could have happened on. Both sides of a payment show the same code,
            so the payer can read theirs to whoever took the money.
          -->
          <div
            v-if="transactions && transactions.items.length"
            class="flex flex-col gap-2 sm:flex-row sm:flex-wrap sm:items-end"
          >
            <UFormField
              :label="$t('profile.referenceCodeLabel')"
              class="min-w-0 flex-1"
            >
              <UInput
                v-model="txQuery"
                icon="i-lucide-search"
                :placeholder="$t('profile.referencePlaceholder')"
                class="w-full"
                spellcheck="false"
                autocapitalize="characters"
              />
            </UFormField>
            <div class="flex flex-wrap gap-2">
              <UFormField :label="$t('profile.from')">
                <UInput
                  v-model="txFrom"
                  type="date"
                />
              </UFormField>
              <UFormField :label="$t('profile.to')">
                <UInput
                  v-model="txTo"
                  type="date"
                />
              </UFormField>
            </div>
            <UButton
              v-if="txFiltering"
              :label="$t('common.clear')"
              icon="i-lucide-x"
              color="neutral"
              variant="ghost"
              size="sm"
              @click="clearTxFilters"
            />
          </div>

          <div
            v-if="!linkedWallet"
            class="flex flex-col items-start gap-3 rounded-xl border border-dashed border-default p-4"
          >
            <span class="font-medium">{{ $t('profile.linkToSeeTx') }}</span>
            <p class="text-sm text-muted">
              {{ $t('profile.linkToSeeTxBody') }}
            </p>
          </div>

          <UAlert
            v-else-if="txFailure"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            :title="$t('profile.txErrorTitle')"
            :description="txFailure"
          />

          <p
            v-else-if="txLoading && !transactions"
            class="py-4 text-center text-sm text-muted"
          >
            {{ $t('profile.txLoading') }}
          </p>

          <p
            v-else-if="transactions && transactions.items.length === 0"
            class="py-4 text-center text-sm text-muted"
          >
            {{ $t('profile.txEmpty') }}
          </p>

          <p
            v-else-if="txFiltering && !filteredTransactions.length"
            class="py-4 text-center text-sm text-muted"
          >
            {{ $t('profile.txNoMatch') }}
          </p>

          <UButton
            v-else-if="transactions && !txOpen"
            :label="$t('profile.showPayments', { count: transactions.items.length }, transactions.items.length)"
            icon="i-lucide-chevron-down"
            color="neutral"
            variant="ghost"
            block
            @click="txOpen = true"
          />

          <ul
            v-else-if="transactions"
            class="flex flex-col divide-y divide-default"
          >
            <!--
              `min-w-0` lets the row shrink below the memo's full width: the
              memo is truncated, but its nowrap text would otherwise set this
              flex item's automatic minimum and widen the whole card.
            -->
            <li
              v-for="item in filteredTransactions"
              :key="item.id"
              class="flex min-w-0 items-center gap-3 py-2"
            >
              <UIcon
                :name="item.direction === 'received' ? 'i-lucide-arrow-down-left' : 'i-lucide-arrow-up-right'"
                :class="item.direction === 'received' ? 'text-success size-5' : 'text-error size-5'"
                class="shrink-0"
              />
              <div class="min-w-0 flex-1">
                <p class="truncate text-sm">
                  {{ item.memo || (item.direction === 'received' ? $t('profile.paymentReceived') : $t('profile.paymentSent')) }}
                </p>
                <!-- The code first: it is the part a customer reads out and
                     the part a seller searches for, so it has to be legible. -->
                <p class="truncate text-xs text-muted">
                  <span class="font-mono tracking-wider text-default">{{ referenceCode(item.id) }}</span>
                  · {{ formatDate(item.settled_at_unix || item.created_at_unix) }}
                  · {{ item.direction === 'received' ? $t('profile.directionFrom') : $t('profile.directionTo') }}
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

          <UButton
            v-if="txOpen"
            :label="$t('profile.hide')"
            icon="i-lucide-chevron-up"
            color="neutral"
            variant="ghost"
            block
            @click="txOpen = false"
          />
        </div>
      </UCard>
    </div>
  </UPageSection>
</template>
