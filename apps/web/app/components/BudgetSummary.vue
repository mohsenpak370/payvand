<script setup lang="ts">
import type { SupportedCurrency } from '~/composables/usePaymentApi'

/**
 * The allowance at a glance: this month's ceiling, what is left of it, what the
 * pot holds, and which key pays. The figures follow the currency switch, so
 * each one is written in the units its allowance is held in.
 */
const props = defineProps<{
  currency: SupportedCurrency
  /** The month's ceiling, in base units. */
  limit: number
  /** What is left of the ceiling, in the same units. */
  remaining: number
  /** What the pot or vault holds right now. */
  pot: number
  sessionKey: string
  /** The paying key belongs to another device, or this one was never set up. */
  foreignDevice: boolean
}>()

function amountLabel(value: number): string {
  return budgetAmountLabel(props.currency, value)
}

function shorten(value: string) {
  return `${value.slice(0, 4)}…${value.slice(-4)}`
}
</script>

<template>
  <div class="grid grid-cols-2 gap-4 sm:grid-cols-4">
    <div>
      <p class="text-sm text-muted">
        {{ $t('budget.display.monthlyLimit') }}
      </p>
      <p class="text-lg font-semibold">
        {{ amountLabel(limit) }}
      </p>
    </div>
    <div>
      <p class="text-sm text-muted">
        {{ $t('budget.display.leftThisMonth') }}
      </p>
      <p class="text-lg font-semibold text-success">
        {{ amountLabel(remaining) }}
      </p>
    </div>
    <div>
      <p class="text-sm text-muted">
        {{ $t('budget.display.pot') }}
      </p>
      <p class="text-lg font-semibold">
        {{ amountLabel(pot) }}
      </p>
    </div>
    <div>
      <p class="text-sm text-muted">
        {{ $t('budget.display.sessionKey') }}
      </p>
      <p class="font-mono text-sm">
        {{ shorten(sessionKey) }}
      </p>
      <p
        v-if="foreignDevice"
        class="text-xs text-muted"
      >
        {{ $t('budget.display.anotherDevice') }}
      </p>
    </div>
  </div>
</template>
