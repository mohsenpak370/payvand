<script setup lang="ts">
/**
 * What one allowance signature costs in SOL, row by row: the pot top-up, the
 * session key's float, the account rent when it has to be made, and the
 * network fee. The rows come from `useBudget`'s `open*BudgetCost`, which reads
 * the float from `Config`.
 */
const props = defineProps<{
  cost: { rows: { key: string, lamports: number }[], total: number } | null
}>()

const rows = computed(() => (props.cost?.rows ?? []).filter(row => row.lamports > 0))
</script>

<template>
  <div
    v-if="cost"
    class="rounded-md border border-default p-3"
  >
    <p class="text-xs font-medium text-highlighted">
      {{ $t('budget.cost.title') }}
    </p>
    <dl class="mt-1 flex flex-col gap-0.5 text-xs text-muted tabular-nums">
      <div
        v-for="row in rows"
        :key="row.key"
        class="flex justify-between gap-4"
      >
        <dt>{{ $t(`budget.cost.${row.key}`) }}</dt>
        <dd>{{ lamportsToSol(row.lamports) }} SOL</dd>
      </div>
    </dl>
    <p class="mt-1 flex justify-between gap-4 border-t border-default pt-1 text-xs font-medium text-default tabular-nums">
      <span>{{ $t('budget.cost.total') }}</span>
      <span>{{ lamportsToSol(cost.total) }} SOL</span>
    </p>
    <p class="mt-1 text-xs text-muted">
      {{ $t('budget.cost.note') }}
    </p>
  </div>
</template>
