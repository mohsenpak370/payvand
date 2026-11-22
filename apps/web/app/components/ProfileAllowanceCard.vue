<script setup lang="ts">
import type { FiatCurrency } from '~/composables/useFiat'
import type { ProfileAllowanceRow } from '~/composables/useProfileAllowance'

/**
 * The allowance at a glance, one row per currency the cluster can pay in. The
 * three are separate accounts with their own units, so each row stays in its own
 * currency; the total is the one figure that needs a price, and it is written in
 * the currency the person picked on the settings page.
 */
defineProps<{
  hasAddress: boolean
  rows: ProfileAllowanceRow[]
  hasBudget: boolean
  /** Every remaining allowance in the display currency; null without a rate. */
  totalFiat: number | null
  displayCurrency: FiatCurrency
  loading: boolean
  error: string
}>()

function rowAmount(currency: ProfileAllowanceRow['currency'], amount: number): string {
  return budgetAmountLabel(currency, amount)
}
</script>

<template>
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
        v-if="!hasAddress"
        class="text-sm text-muted"
      >
        {{ $t('profile.connectForAllowance') }}
      </p>

      <UAlert
        v-else-if="error"
        color="error"
        variant="subtle"
        icon="i-lucide-circle-alert"
        :title="$t('profile.allowanceErrorTitle')"
        :description="error"
      />

      <p
        v-else-if="loading"
        class="py-4 text-center text-sm text-muted"
      >
        {{ $t('profile.allowanceLoading') }}
      </p>

      <!-- The prompt: none of the three has an allowance yet. -->
      <div
        v-else-if="!hasBudget"
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

      <!-- The status: one row per currency, then the total in fiat. -->
      <template v-else>
        <div class="grid gap-3 sm:grid-cols-3">
          <div
            v-for="row in rows"
            :key="row.currency"
            class="flex min-w-0 flex-col gap-1 rounded-xl border border-default p-3"
          >
            <div class="flex items-center justify-between gap-2">
              <span class="text-sm font-medium">{{ currencySymbol(row.currency) }}</span>
              <UBadge
                v-if="!row.set"
                color="neutral"
                variant="subtle"
                size="sm"
              >
                {{ $t('profile.notSet') }}
              </UBadge>
            </div>
            <span class="text-xl font-semibold tabular-nums sm:text-2xl">
              {{ rowAmount(row.currency, row.remaining) }}
            </span>
            <span class="text-xs text-muted">{{ $t('profile.remaining') }}</span>
            <span class="text-xs text-muted tabular-nums">
              {{ $t('profile.limitAndSpent', {
                limit: rowAmount(row.currency, row.limit),
                spent: rowAmount(row.currency, row.spent)
              }) }}
            </span>
            <span
              v-if="row.set"
              class="text-xs text-muted tabular-nums"
            >
              {{ $t('profile.potLine', { amount: rowAmount(row.currency, row.pot) }) }}
            </span>
          </div>
        </div>

        <div class="border-default flex flex-wrap items-center justify-between gap-3 border-t pt-4">
          <div class="flex min-w-0 flex-col">
            <span class="text-sm text-muted">{{ $t('profile.totalThisMonth') }}</span>
            <span class="text-lg font-semibold tabular-nums">
              {{ totalFiat === null ? '—' : formatFiat(totalFiat, displayCurrency) }}
            </span>
          </div>
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
</template>
