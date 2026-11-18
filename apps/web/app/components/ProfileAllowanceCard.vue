<script setup lang="ts">
import type { BudgetState } from '~/composables/useBudget'

/**
 * The allowance at a glance. useProfileAllowance reads the chain and derives
 * the figures; this card only decides which of its four states to show.
 */
defineProps<{
  hasAddress: boolean
  budget: BudgetState | null
  budgetPot: number
  loading: boolean
  error: string
  spentThisMonth: number
  remaining: number
  spentPercent: number
}>()
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
</template>
