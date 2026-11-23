<script setup lang="ts">
import type { MyTransaction, TransactionsResponse } from '~/composables/useAuth'
import { currencySymbol, formatTokenAmount } from '~/composables/usePaymentApi'

/**
 * The wallet's payment history. The fetch lives in useProfileTransactions;
 * finding one payment is this card's own concern, so the search fields and the
 * derived list stay here next to the rows they filter.
 */
const props = defineProps<{
  linkedWallet: string | null
  transactions: TransactionsResponse | null
  loading: boolean
  failure: string
}>()

const emit = defineEmits<{ refresh: [] }>()

const { t } = useI18n()

// Both lists run long. The saved codes start folded, so someone who came for
// the wallet or the allowance is not scrolled past them — but the payments are
// the record of what happened and who it happened with, so they start open.
const txOpen = ref(true)

const txQuery = ref('')
const txFrom = ref('')
const txTo = ref('')
// The dates live in a popover next to the search field: they are the occasional
// filter, while the code is what someone reaches for every time.
const filtersOpen = ref(false)

// One payment, opened over the list. Everything shown here is what the row
// already knows plus the fee, which is quoted from the same arithmetic the
// checkout uses rather than stored per payment.
const selected = ref<MyTransaction | null>(null)
const detailFee = ref<string | null>(null)
const { estimateInvoiceCosts } = useInvoiceCosts()

const detailOpen = computed({
  get: () => selected.value !== null,
  set: (open: boolean) => {
    if (!open) selected.value = null
  }
})

async function openDetail(item: MyTransaction) {
  selected.value = item
  detailFee.value = null
  try {
    const mint = item.currency === 'sol'
      ? undefined
      : (mintForCurrency(item.currency, props.transactions?.cluster ?? '') ?? undefined)
    const quote = await estimateInvoiceCosts(item.amount, item.currency, { mint })
    // The list may have moved on while the quote read the chain.
    if (selected.value?.id === item.id) {
      detailFee.value = `${formatTokenAmount(quote.fee, item.decimals)} ${currencySymbol(item.currency)}`
    }
  } catch {
    detailFee.value = null
  }
}

/** Every line of the detail sheet, in the order a receipt reads. */
const detailRows = computed(() => {
  const item = selected.value
  if (!item) return []
  const rows: { label: string, value: string, mono?: boolean }[] = []
  if (item.name) rows.push({ label: t('profile.detailName'), value: item.name })
  if (item.description) rows.push({ label: t('profile.detailDescription'), value: item.description })
  if (item.memo) rows.push({ label: t('profile.detailMemo'), value: item.memo })
  rows.push({
    label: t('profile.detailAmount'),
    value: `${item.direction === 'received' ? '+' : '−'}${formatTokenAmount(item.amount, item.decimals)} ${currencySymbol(item.currency)}`
  })
  rows.push({ label: t('profile.detailFee'), value: detailFee.value ?? '—' })
  rows.push({
    label: t('profile.detailPayer'),
    value: item.direction === 'received'
      ? item.counterparty
      : (props.linkedWallet ?? item.counterparty)
  })
  rows.push({
    label: t('profile.detailPayee'),
    value: item.direction === 'received'
      ? (props.linkedWallet ?? item.counterparty)
      : item.counterparty
  })
  if (item.settled_at_unix) {
    rows.push({ label: t('profile.detailDate'), value: formatDate(item.settled_at_unix) })
    rows.push({ label: t('profile.detailCreated'), value: formatDate(item.created_at_unix) })
  } else {
    rows.push({ label: t('profile.detailCreated'), value: formatDate(item.created_at_unix) })
  }
  rows.push({ label: t('profile.detailReference'), value: item.id, mono: true })
  return rows
})

/** The day input as unix seconds, at the start or the end of that day. */
function dayBound(date: string, end: boolean): number | null {
  if (!date) return null
  const at = new Date(`${date}T00:00:00`)
  if (Number.isNaN(at.getTime())) return null
  return Math.floor(at.getTime() / 1000) + (end ? 86_399 : 0)
}

const dateFiltersActive = computed(() => Boolean(txFrom.value || txTo.value))

const txFiltering = computed(() => Boolean(txQuery.value.trim() || dateFiltersActive.value))

const filteredTransactions = computed(() => {
  const items = props.transactions?.items ?? []
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

function clearDates() {
  txFrom.value = ''
  txTo.value = ''
}

function statusColor(status: string): 'success' | 'warning' | 'neutral' {
  return status === 'settled'
    ? 'success'
    : status === 'pending' || status === 'missing'
      ? 'warning'
      : 'neutral'
}

/** "1 payment" / "2 payments", in whatever forms the locale defines. */
function paymentCount(count: number, shown?: number) {
  return shown === undefined
    ? t('profile.txCount', { count, cluster: networkLabel(props.transactions?.cluster ?? '') }, count)
    : t('profile.txCountFiltered', {
        shown,
        total: count,
        cluster: networkLabel(props.transactions?.cluster ?? '')
      }, count)
}
</script>

<template>
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
          :loading="loading"
          @click="emit('refresh')"
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
        class="flex items-end gap-2"
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

        <!--
          The dates sit one click away instead of taking a row of their own:
          the code is the daily filter, the dates are the exception. The button
          turns primary while a date is narrowing the list, so an empty result
          never hides the reason for it.
        -->
        <UPopover v-model:open="filtersOpen">
          <UButton
            :label="$t('profile.filters')"
            icon="i-lucide-list-filter"
            :color="dateFiltersActive ? 'primary' : 'neutral'"
            variant="soft"
            :aria-label="$t('profile.filters')"
          />
          <template #content>
            <div class="flex w-72 flex-col gap-3 p-4">
              <UFormField :label="$t('profile.from')">
                <UInput
                  v-model="txFrom"
                  type="date"
                  class="w-full"
                />
              </UFormField>
              <UFormField :label="$t('profile.to')">
                <UInput
                  v-model="txTo"
                  type="date"
                  class="w-full"
                />
              </UFormField>
              <div class="flex items-center justify-between gap-2">
                <UButton
                  :label="$t('profile.clearDates')"
                  color="neutral"
                  variant="ghost"
                  size="sm"
                  :disabled="!dateFiltersActive"
                  @click="clearDates"
                />
                <UButton
                  :label="$t('common.done')"
                  size="sm"
                  @click="filtersOpen = false"
                />
              </div>
            </div>
          </template>
        </UPopover>

        <UButton
          v-if="txFiltering"
          :label="$t('common.clear')"
          icon="i-lucide-x"
          color="neutral"
          variant="ghost"
          size="sm"
          class="shrink-0"
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
        v-else-if="failure"
        color="error"
        variant="subtle"
        icon="i-lucide-circle-alert"
        :title="$t('profile.txErrorTitle')"
        :description="failure"
      />

      <p
        v-else-if="loading && !transactions"
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
          class="min-w-0"
        >
          <!-- The row opens the whole payment: what it was for, what it cost
               and who it was between. -->
          <button
            type="button"
            class="-mx-2 flex w-[calc(100%+1rem)] min-w-0 items-center gap-3 rounded-xl px-2 py-2 text-start transition-colors hover:bg-elevated/60"
            :aria-label="`${t('profile.detailTitle')} · ${referenceCode(item.id)}`"
            @click="openDetail(item)"
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
                {{ item.direction === 'received' ? '+' : '−' }}{{ formatTokenAmount(item.amount, item.decimals) }} {{ currencySymbol(item.currency) }}
              </span>
              <UBadge
                :color="statusColor(item.status)"
                variant="subtle"
                size="xs"
              >
                {{ statusLabel(item.status) }}
              </UBadge>
            </div>
          </button>
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

    <UModal
      v-model:open="detailOpen"
      :title="$t('profile.detailTitle')"
      :description="selected ? referenceCode(selected.id) : undefined"
    >
      <template #body>
        <div
          v-if="selected"
          class="flex flex-col gap-4"
        >
          <div class="flex items-center justify-between gap-3">
            <span class="text-sm text-muted">{{ $t('profile.detailStatus') }}</span>
            <UBadge
              :color="statusColor(selected.status)"
              variant="subtle"
              size="sm"
            >
              {{ statusLabel(selected.status) }}
            </UBadge>
          </div>

          <dl class="flex flex-col divide-y divide-default rounded-xl border border-default">
            <div
              v-for="row in detailRows"
              :key="row.label"
              class="flex items-baseline justify-between gap-3 px-3 py-2 text-sm"
            >
              <dt class="shrink-0 text-muted">
                {{ row.label }}
              </dt>
              <dd
                class="min-w-0 text-end"
                :class="row.mono ? 'break-all font-mono text-xs' : ''"
              >
                {{ row.value }}
              </dd>
            </div>
          </dl>

          <p class="text-xs text-muted">
            {{ $t('profile.detailFeeNote') }}
          </p>
        </div>
      </template>

      <template #footer>
        <UButton
          :label="$t('profile.detailClose')"
          class="ms-auto"
          @click="detailOpen = false"
        />
      </template>
    </UModal>
  </UCard>
</template>
