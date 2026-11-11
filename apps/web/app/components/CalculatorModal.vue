<script setup lang="ts">
// A price is often arithmetic — "three of these at 0.015" — so the amount
// field can borrow a calculator instead of sending the seller to another app
// to work it out and retype the answer.

const open = defineModel<boolean>('open', { required: true })

const props = defineProps<{
  /** The amount already in the price field, so a seller can carry on from it. */
  initial?: string
}>()

const emit = defineEmits<{ apply: [amount: string] }>()

type Operator = '+' | '-' | '×' | '÷'

interface Key {
  key: string
  label?: string
  icon?: string
  /** Locale key for the spoken name of an icon-only key. */
  ariaKey?: string
  tone?: 'operator' | 'equals'
  /** Zero spans three cells, leaving the last one for the decimal point. */
  wide?: boolean
}

// Reading order, four columns. "0" takes three cells so the decimal point can
// sit alone on the last row, the way a phone keypad does.
const ROWS: Key[][] = [
  [
    { key: 'C', label: 'C' },
    { key: '⌫', icon: 'i-lucide-delete', ariaKey: 'calculator.deleteLastDigit' },
    { key: '÷', label: '÷', tone: 'operator' },
    { key: '×', label: '×', tone: 'operator' }
  ],
  [
    { key: '7' },
    { key: '8' },
    { key: '9' },
    { key: '-', label: '−', tone: 'operator' }
  ],
  [
    { key: '4' },
    { key: '5' },
    { key: '6' },
    { key: '+', label: '+', tone: 'operator' }
  ],
  [
    { key: '1' },
    { key: '2' },
    { key: '3' },
    { key: '=', label: '=', tone: 'equals' }
  ],
  [
    { key: '0', wide: true },
    { key: '.', label: '.' }
  ]
]

const OPERATORS: Operator[] = ['+', '-', '×', '÷']

const MAX_DIGITS = 12

// `entry` is the number on screen, `stored` the left-hand side of a pending
// operation, and `fresh` marks an entry the next digit should replace.
const entry = ref('0')
const stored = ref<number | null>(null)
const pending = ref<Operator | null>(null)
const fresh = ref(true)

const value = computed(() => Number(entry.value))
const canApply = computed(() => Number.isFinite(value.value) && value.value > 0)
const expression = computed(() =>
  pending.value !== null && stored.value !== null
    ? `${trim(stored.value)} ${pending.value}${fresh.value ? '' : ` ${entry.value}`}`
    : ''
)

// SOL has nine decimals; rounding here keeps 0.1 + 0.2 from showing as
// 0.30000000000000004.
function trim(amount: number): string {
  return String(Number(amount.toFixed(9)))
}

function compute(left: number, operator: Operator, right: number): number {
  if (operator === '+') return left + right
  if (operator === '-') return left - right
  if (operator === '×') return left * right
  return right === 0 ? Number.NaN : left / right
}

function reset(seed = '') {
  const start = Number(seed)
  entry.value = Number.isFinite(start) && start > 0 ? trim(start) : '0'
  stored.value = null
  pending.value = null
  fresh.value = true
}

function digit(key: string) {
  if (fresh.value) {
    entry.value = key
    fresh.value = false
    return
  }
  if (entry.value.replace('.', '').length < MAX_DIGITS) entry.value += key
}

function point() {
  if (fresh.value) {
    entry.value = '0.'
    fresh.value = false
    return
  }
  if (!entry.value.includes('.')) entry.value += '.'
}

function backspace() {
  entry.value = entry.value.length > 1 ? entry.value.slice(0, -1) : '0'
  if (entry.value === '0') fresh.value = true
}

function operator(next: Operator) {
  // Choosing a second operator folds the pending one in, so 2 × 3 + 4
  // behaves like a pocket calculator.
  if (pending.value && stored.value !== null && !fresh.value) {
    const folded = compute(stored.value, pending.value, value.value)
    if (!Number.isFinite(folded)) {
      reset()
      return
    }
    entry.value = trim(folded)
  }
  stored.value = Number(entry.value)
  pending.value = next
  fresh.value = true
}

function equals() {
  if (!pending.value || stored.value === null) return
  const result = compute(stored.value, pending.value, value.value)
  entry.value = Number.isFinite(result) ? trim(result) : '0'
  stored.value = null
  pending.value = null
  fresh.value = true
}

function press(key: string) {
  if (/^[0-9]$/.test(key)) return digit(key)
  if (key === '.') return point()
  if (key === 'C') return reset()
  if (key === '⌫') return backspace()
  if (key === '=') return equals()
  if (OPERATORS.includes(key as Operator)) operator(key as Operator)
}

function tone(key: Key) {
  if (key.tone === 'equals') return { color: 'primary' as const, variant: 'solid' as const }
  if (key.tone === 'operator') return { color: 'primary' as const, variant: 'soft' as const }
  return { color: 'neutral' as const, variant: 'soft' as const }
}

function apply() {
  if (!canApply.value) return
  emit('apply', trim(value.value))
  open.value = false
}

// Opening starts from whatever the price field already holds.
watch(open, (isOpen) => {
  if (isOpen) reset(props.initial ?? '')
})
</script>

<template>
  <!--
    No transition: with Nuxt UI's exit animation this modal closed in state but
    stayed painted over the form with the body scroll lock still on, so the
    panel unmounts as soon as `open` flips instead.
  -->
  <UModal
    v-model:open="open"
    :title="$t('calculator.title')"
    :transition="false"
    :ui="{ content: 'max-w-xs' }"
  >
    <template #body>
      <div class="flex flex-col gap-3">
        <div class="rounded-lg bg-elevated px-4 py-3 text-right">
          <p class="h-4 text-xs text-muted">
            {{ expression }}
          </p>
          <p class="truncate text-3xl font-semibold tabular-nums">
            {{ entry }}
          </p>
        </div>

        <div class="grid grid-cols-4 gap-2">
          <template
            v-for="(row, index) in ROWS"
            :key="index"
          >
            <UButton
              v-for="k in row"
              :key="k.key"
              :label="k.icon ? undefined : (k.label ?? k.key)"
              :icon="k.icon"
              :aria-label="k.ariaKey ? $t(k.ariaKey) : undefined"
              class="h-11"
              :class="k.wide ? 'col-span-3' : undefined"
              size="lg"
              block
              v-bind="tone(k)"
              @click="press(k.key)"
            />
          </template>
        </div>
      </div>
    </template>

    <template #footer>
      <div class="flex w-full items-center justify-between gap-3">
        <!--
          The bare number: the amount field decides the unit (SOL or the
          seller's currency), so naming one here would be wrong half the time.
        -->
        <p class="text-sm text-muted tabular-nums">
          {{ canApply ? trim(value) : $t('calculator.enterAmount') }}
        </p>
        <UButton
          :label="$t('calculator.useAmount')"
          :disabled="!canApply"
          @click="apply"
        />
      </div>
    </template>
  </UModal>
</template>
