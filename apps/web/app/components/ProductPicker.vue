<script setup lang="ts">
import type { CartLine, Product, ShopWithProducts } from '~/composables/useShop'

// Tapping products is the whole point: a seller behind a counter should price a
// payment by quantity, not by adding up numbers in their head. The total here
// is what the invoice charges, and each line becomes the buyer's receipt.
const props = defineProps<{ shops: ShopWithProducts[] }>()
const lines = defineModel<CartLine[]>({ required: true })

/** Shops with something to sell, in the order the profile lists them. */
const shelves = computed(() => props.shops.filter(shop => shop.products.length > 0))
/** Every product on every shelf, for keeping the cart in menu order. */
const allProducts = computed(() => shelves.value.flatMap(shop => shop.products))

// Mirrors `MAX_ITEMS` in the API: every line ends up inside the QR code, so a
// receipt that added one more would stop being scannable.
const MAX_LINES = 8
const atLineLimit = computed(() => lines.value.length >= MAX_LINES)

const { currency } = useCurrencyPreference()
const rates = useSolRates()

const total = computed(() =>
  lines.value.reduce((sum, line) => sum + line.product.price_lamports * line.quantity, 0)
)

/** A product's line in the cart, if it has one. */
function lineFor(product: Product): CartLine | undefined {
  return lines.value.find(line => line.product.id === product.id)
}

function quantityFor(product: Product): number {
  return lineFor(product)?.quantity ?? 0
}

/**
 * Sets a product's quantity, keeping the cart in list order so the receipt
 * reads like the menu. Zero removes the line rather than keeping a ghost.
 */
function setQuantity(product: Product, quantity: number) {
  const next = Math.max(0, Math.min(99, quantity))
  const without = lines.value.filter(line => line.product.id !== product.id)
  if (next === 0) {
    lines.value = without
    return
  }
  const ordered = allProducts.value
    .map(item => (item.id === product.id
      ? { product, quantity: next }
      : without.find(line => line.product.id === item.id)))
    .filter((line): line is CartLine => Boolean(line))
  lines.value = ordered
}

function increase(product: Product) {
  setQuantity(product, quantityFor(product) + 1)
}

function decrease(product: Product) {
  setQuantity(product, quantityFor(product) - 1)
}

function clear() {
  lines.value = []
}

const fiatTotal = computed(() => fiatFromLamports(total.value, rates.value, currency.value))
</script>

<template>
  <div class="flex flex-col gap-3 rounded-xl border border-default p-3">
    <div class="flex items-center justify-between gap-3">
      <span class="text-sm font-medium">From your shops</span>
      <UButton
        v-if="lines.length"
        label="Clear"
        color="neutral"
        variant="ghost"
        size="xs"
        @click="clear"
      />
    </div>

    <ul
      v-for="shop in shelves"
      :key="shop.id"
      class="flex flex-col divide-y divide-default"
    >
      <li class="pt-2 pb-1 text-xs font-medium text-muted">
        {{ shop.name }}
      </li>
      <li
        v-for="product in shop.products"
        :key="product.id"
        class="flex min-w-0 items-center justify-between gap-3 py-2"
      >
        <div class="min-w-0">
          <p class="text-sm break-words">
            {{ product.name }}
          </p>
          <p class="text-xs text-muted tabular-nums">
            {{ lamportsToSol(product.price_lamports) }} SOL
          </p>
        </div>

        <div class="flex shrink-0 items-center gap-1">
          <UButton
            icon="i-lucide-minus"
            color="neutral"
            variant="outline"
            size="xs"
            square
            :disabled="quantityFor(product) === 0"
            :aria-label="`One fewer ${product.name}`"
            @click="decrease(product)"
          />
          <span class="w-6 text-center text-sm tabular-nums">{{ quantityFor(product) }}</span>
          <UButton
            icon="i-lucide-plus"
            color="neutral"
            variant="outline"
            size="xs"
            square
            :aria-label="`One more ${product.name}`"
            :disabled="quantityFor(product) === 0 && atLineLimit"
            @click="increase(product)"
          />
        </div>
      </li>
    </ul>

    <div class="flex items-baseline justify-between gap-3 border-t border-default pt-2 text-sm">
      <span class="font-medium">Total</span>
      <span class="tabular-nums">
        {{ lamportsToSol(total) }} SOL
        <span
          v-if="fiatTotal"
          class="text-muted"
        >(≈ {{ fiatTotal }})</span>
      </span>
    </div>

    <p
      v-if="atLineLimit"
      class="text-xs text-muted"
    >
      A receipt fits {{ MAX_LINES }} different products — make a second payment for more.
    </p>
  </div>
</template>
