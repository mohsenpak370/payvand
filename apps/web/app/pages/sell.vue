<script setup lang="ts">
import { address } from '@vue-solana/nuxt/kit'
import type {
  DiscountCode,
  PaymentItem,
  PaymentRecord,
  SupportedCurrency
} from '~/composables/usePaymentApi'
import { currencySymbol, formatTokenAmount } from '~/composables/usePaymentApi'
import { currenciesForCluster } from '~/composables/usePaymentProgram'
import type { FiatCurrency } from '~/composables/useFiat'
import type { CartLine, ShopWithProducts } from '~/composables/useShop'
import { cartTotalLamports } from '~/composables/useShop'

useSeoMeta({ title: 'Sell — Payvand' })

definePageMeta({ middleware: 'auth' })

const { cluster } = useRuntimeConfig().public
const api = usePaymentApi()
const shopApi = useShopApi()
const { publicKey, connected } = useSolanaWallet()

const amountSol = ref('0.01')
// Sellers think in the money they charge; the chain takes SOL. The amount can
// be typed in either, and the other one is shown next to it, but every invoice
// is created in lamports.
const { currency, setCurrency } = useCurrencyPreference()
const rates = useSolRates()
const priceInFiat = ref(false)
const fiatAmount = ref('')
// The one currency this invoice charges in. Offering several prices at once
// only hands the customer a decision; the seller decides, and the code says it.
const priceCurrency = ref<SupportedCurrency>('sol')
const currencyChoices = computed(() => currenciesForCluster(cluster))
const amountUnits = [
  { label: 'SOL', value: 'sol' },
  { label: 'USD', value: 'usd' },
  { label: 'EUR', value: 'eur' }
]
// The amount field opens a calculator: sellers often price from arithmetic
// rather than a number they already know.
const calculatorOpen = ref(false)
const name = ref('')
const description = ref('')
const memo = ref('Coffee')
const busy = ref(false)
const failure = ref('')
const record = ref<PaymentRecord | null>(null)
const qrPayload = ref('')
// The receipt this payment carries. Kept here rather than read back off the
// record: a status refresh replaces the record with the chain's copy, which
// knows the memo and the amount but nothing about what was sold.
const createdItems = ref<PaymentItem[]>([])

// The seller's own menus, offered as taps instead of typing. An account with
// no products simply never sees this block.
const shops = ref<ShopWithProducts[]>([])
const hasProducts = computed(() => shops.value.some(shop => shop.products.length > 0))
const cart = ref<CartLine[]>([])
const cartTotal = computed(() => cartTotalLamports(cart.value))
const cartActive = computed(() => cart.value.length > 0)
// An invoice is priced one of two ways: a number the seller types, or the total
// of what they pick off a shop's menu. Most codes are the first, so the menus
// stay folded away until a seller asks for them.
const priceMode = ref<'amount' | 'products'>('amount')
const priceModeItems = [
  { label: 'Type an amount', value: 'amount' },
  { label: 'From my shops', value: 'products' }
]
const sellingFromShops = computed(() => priceMode.value === 'products')

// Discount codes, and the price of the open invoice — after settlement the
// chain reports the *received* amount, so the full price is kept here.
const discounts = ref<DiscountCode[]>([])
const newCode = ref('')
const newPercent = ref('10')
const codeBusy = ref(false)
const codeFailure = ref('')
const priceLamports = ref(0)
const paidCode = ref<string | null>(null)
// A code carried by the invoice itself: written into its QR, priced here from
// the seller's own list, and applied by the buyer's screen without typing.
// `NONE` rather than '': the select control reserves the empty string for its
// placeholder and refuses it as an option value.
const NONE = 'none'
const discountCode = ref(NONE)

const selectedDiscount = computed(
  () => discounts.value.find(code => code.code === discountCode.value) ?? null
)

const discountOptions = computed(() => [
  { label: 'No discount', value: NONE },
  ...discounts.value.map(code => ({
    label: `${code.code} · ${code.percent_off}% off`,
    value: code.code
  }))
])

const canAddCode = computed(() =>
  !codeBusy.value
  && newCode.value.trim().length >= 3
  && Number(newPercent.value) >= 1
  && Number(newPercent.value) <= 99
)

const lamports = computed(() => {
  if (sellingFromShops.value) return cartTotal.value
  return priceInFiat.value
    ? lamportsFromFiat(Number(fiatAmount.value), rates.value, currency.value)
    : solToLamports(Number(amountSol.value))
})

/** SOL, or the currency the price is typed in. Picking a currency also makes it the default. */
const amountUnit = computed<'sol' | FiatCurrency>({
  get: () => (priceInFiat.value ? currency.value : 'sol'),
  set: (next) => {
    if (next === 'sol') {
      // Carry the number over so switching unit never changes the price.
      if (priceInFiat.value) amountSol.value = String(Number((lamports.value / 1_000_000_000).toFixed(9)))
      priceInFiat.value = false
      return
    }
    const rate = rates.value?.[next]
    const equivalent = rate ? (lamports.value / 1_000_000_000) * rate : 0
    fiatAmount.value = equivalent ? String(Number(equivalent.toFixed(2))) : ''
    priceInFiat.value = true
    setCurrency(next)
  }
})
// Creating a code needs the seller's address, never a signature: the invoice
// address is derived locally and the account only comes into existence (and
// is only funded) when the customer pays.
// A price in SOL converts to the stablecoins at today's rate, so a ticked
// stablecoin with no rate available cannot be priced — say so instead of
// creating an invoice that charges zero.
const usdAmount = computed(() => {
  if (priceInFiat.value && currency.value === 'usd') return Number(fiatAmount.value) || 0
  const rate = rates.value?.usd
  return rate ? (lamports.value / 1_000_000_000) * rate : 0
})
const eurAmount = computed(() => {
  if (priceInFiat.value && currency.value === 'eur') return Number(fiatAmount.value) || 0
  const rate = rates.value?.eur
  return rate ? (lamports.value / 1_000_000_000) * rate : 0
})

/** The invoice's price in its own base units — exactly what the code charges. */
const invoiceOptions = computed(() => {
  const option = priceCurrency.value
  if (option === 'sol') return [{ currency: option, amount: lamports.value }]
  const fiat = option === 'usdc' ? usdAmount.value : eurAmount.value
  return [{ currency: option, amount: Math.round(fiat * 1_000_000) }]
})

/** A price in the money the customer will hand over. */
function priceLabel(option: { currency: SupportedCurrency, amount: number }): string {
  return option.currency === 'sol'
    ? `${lamportsToSol(option.amount)} SOL`
    : `${formatTokenAmount(option.amount, 6)} ${currencySymbol(option.currency)}`
}

/** The price shown next to the form, and the same price after a carried code. */
const priceSummary = computed(() => priceLabel(invoiceOptions.value[0]!))
const discountedPrice = computed(() => {
  const option = invoiceOptions.value[0]!
  if (!selectedDiscount.value) return priceLabel(option)
  return priceLabel({
    currency: option.currency,
    amount: discountedLamports(option.amount, selectedDiscount.value.percent_off)
  })
})

/** The amount field prices the invoice; a stablecoin invoice is pinned to that price. */
const amountHelp = computed(() => {
  if (cartActive.value) return 'The total of the products above — clear them to type an amount instead.'
  if (priceCurrency.value !== 'sol') {
    return `Priced here in SOL or your own currency, charged in ${currencySymbol(priceCurrency.value)} at today's rate — the exact price is below.`
  }
  return priceInFiat.value
    ? 'Charged in SOL at today\'s rate — the invoice keeps the amount below.'
    : 'What the customer pays.'
})

// Picking products decides the price: the invoice has to charge exactly what
// the receipt adds up to, or the API refuses to build the code.
watch(cartTotal, (total) => {
  if (total <= 0) return
  priceInFiat.value = false
  amountSol.value = String(Number((total / 1_000_000_000).toFixed(9)))
})

// Switching back to a typed amount drops the tray: a cart left behind would
// price the invoice from items nobody can see any more.
watch(priceMode, (mode) => {
  if (mode === 'amount') cart.value = []
})

const canCreate = computed(() =>
  connected.value
  && !busy.value
  && lamports.value > 0
  && invoiceOptions.value.every(option => option.amount > 0)
)

const statusColor = (status: string) =>
  status === 'settled' ? 'success' : status === 'missing' || status === 'pending' ? 'warning' : 'neutral'

function applyAmount(amount: string) {
  if (priceInFiat.value) fiatAmount.value = amount
  else amountSol.value = amount
}

/**
 * Stamps the seller's currency into the invoice code, so the price a customer
 * opens is read in the money the seller charges in — on any phone, signed in
 * or not. The API builds the rest of the payload; this only adds the field.
 */
function withSellerCurrency(payloadJson: string): string {
  try {
    return JSON.stringify({ ...JSON.parse(payloadJson), currency: currency.value })
  } catch {
    return payloadJson
  }
}

async function createPayment() {
  failure.value = ''
  record.value = null
  qrPayload.value = ''

  const seller = publicKey.value
  if (!seller) {
    failure.value = 'Connect your seller wallet first.'
    return
  }
  if (!Number.isFinite(lamports.value) || lamports.value <= 0) {
    failure.value = 'Enter an amount greater than 0.'
    return
  }
  if (memo.value.length > 96) {
    failure.value = 'The note is too long — please shorten it.'
    return
  }
  if (name.value.trim().length > 64) {
    failure.value = 'The name is too long — please shorten it.'
    return
  }
  if (description.value.trim().length > 160) {
    failure.value = 'The description is too long — please shorten it.'
    return
  }

  busy.value = true
  try {
    // Nothing to sign: derive the invoice address from the amount locally,
    // then record the details. The customer's payment funds the account.
    const reference = createReference()

    // The API derives the invoice address from this price, so the code the
    // customer scans names exactly the currency and amount it charges.
    const created = await api.createPayment({
      seller,
      reference,
      options: invoiceOptions.value,
      memo: memo.value,
      name: name.value.trim(),
      description: description.value.trim(),
      items: cart.value.map(line => ({
        name: line.product.name,
        quantity: line.quantity,
        amount_lamports: line.product.price_lamports
      })),
      discount_code: selectedDiscount.value?.code,
      cluster
    })

    record.value = created.payment
    qrPayload.value = withSellerCurrency(created.qrPayload)
    priceLamports.value = lamports.value
    paidCode.value = null
    createdItems.value = created.payment.items ?? []
    // The invoice is made; the next one starts from an empty tray.
    cart.value = []
  } catch (error) {
    failure.value = error instanceof Error ? error.message : 'Could not create the payment.'
  } finally {
    busy.value = false
  }
}

/**
 * The invoice address plus one address per discount code: the same seller
 * and reference priced at each possible amount. Whichever address gets
 * funded tells us the full price — or exactly which code the buyer used.
 */
/**
 * Every address this invoice can be settled on: its currency at full price,
 * plus the same currency at each discount code's price. Whichever gets funded
 * says what was paid and which code the buyer used.
 */
async function watchTargets(): Promise<{ pda: string, code: string | null }[]> {
  const current = record.value
  if (!current) return []

  const seller = address(current.seller)
  const reference = address(current.reference)
  const options = current.options?.length
    ? current.options
    : [{
        currency: 'sol' as SupportedCurrency,
        mint: '',
        decimals: 9,
        amount: current.amount_lamports,
        payment_pda: current.payment_pda
      }]

  const targets: { pda: string, code: string | null }[] = []
  for (const option of options) {
    targets.push({ pda: option.payment_pda, code: null })
    for (const discount of discounts.value) {
      const amount = discountedLamports(option.amount, discount.percent_off)
      if (amount <= 0 || amount === option.amount) continue
      const pda = option.currency === 'sol'
        ? await derivePaymentPda(seller, reference, amount)
        : await deriveTokenPaymentPda(seller, reference, address(option.mint), amount)
      targets.push({ pda: pda.toString(), code: discount.code })
    }
  }
  return targets
}

/** How to print whatever an invoice was settled for, in its own currency. */
function paidAmountLabel(record: PaymentRecord): string {
  const option = record.options?.[0]
  if (option && option.currency !== 'sol') {
    return `${formatTokenAmount(option.amount, option.decimals)} ${currencySymbol(option.currency)}`
  }
  return `${lamportsToSol(record.amount_lamports)} SOL`
}

async function refreshStatus() {
  const current = record.value
  if (!current || current.status === 'settled') return
  try {
    const targets = await watchTargets()
    const results = await api.watchPayments(
      targets.map(target => target.pda),
      { cluster }
    )

    const settledIndex = results.findIndex(result => result.status === 'settled')
    const settled = settledIndex >= 0 ? results[settledIndex] : undefined
    if (settled) {
      // The chain knows the memo but not the name or description, so keep
      // this invoice's own copy once the settled record replaces it.
      record.value = {
        ...settled,
        name: settled.name || current.name,
        description: settled.description || current.description
      }
      paidCode.value = targets[settledIndex]?.code ?? null
      return
    }

    // Nothing paid yet: keep the canonical details, refresh the badge.
    record.value = {
      ...current,
      status: results.some(result => result.status === 'pending') ? 'pending' : 'missing',
      settled_by: null,
      settled_at_unix: 0
    }
  } catch {
    // Keep the last known status; the next tick retries.
  }
}

async function loadDiscounts() {
  try {
    discounts.value = await api.listDiscounts()
  } catch {
    // Signed out elsewhere — the middleware already guards this page.
  }
}

/** The menus, if this account has any. Shops that fail to load must never stop
 *  a seller from typing an amount the way they always could. */
async function loadShop() {
  try {
    shops.value = await shopApi.list()
  } catch {
    shops.value = []
  }
}

async function addDiscount() {
  codeFailure.value = ''
  codeBusy.value = true
  try {
    const created = await api.createDiscount({
      code: newCode.value,
      percent_off: Number(newPercent.value)
    })
    discounts.value = [created, ...discounts.value]
    newCode.value = ''
  } catch (error) {
    codeFailure.value = apiErrorMessage(error, 'Could not add the discount code.')
  } finally {
    codeBusy.value = false
  }
}

async function removeDiscount(discount: DiscountCode) {
  codeFailure.value = ''
  try {
    await api.deleteDiscount(discount.id)
    discounts.value = discounts.value.filter(code => code.id !== discount.id)
  } catch (error) {
    codeFailure.value = apiErrorMessage(error, 'Could not delete the discount code.')
  }
}

function reset() {
  record.value = null
  qrPayload.value = ''
  failure.value = ''
  paidCode.value = null
  priceLamports.value = 0
  createdItems.value = []
}

const copiedCode = ref(false)
const copiedLink = ref(false)
const payloadBox = ref<HTMLElement>()
let copiedTimer: ReturnType<typeof setTimeout> | undefined
let linkTimer: ReturnType<typeof setTimeout> | undefined

/**
 * Copying is the whole point of showing the code, so a refused clipboard must
 * not fail silently: the payload is selected instead, leaving one keystroke.
 */
async function copyPayload() {
  try {
    await navigator.clipboard.writeText(qrPayload.value)
  } catch {
    const code = payloadBox.value?.querySelector('code')
    if (code) {
      const range = document.createRange()
      range.selectNodeContents(code)
      const selection = window.getSelection()
      selection?.removeAllRanges()
      selection?.addRange(range)
    }
    return
  }

  copiedCode.value = true
  clearTimeout(copiedTimer)
  copiedTimer = setTimeout(() => {
    copiedCode.value = false
  }, 2_000)
}

/** The public page this invoice opens — the link to share with the buyer. */
const shareLink = computed(() => (qrPayload.value ? paymentUrl(qrPayload.value) : ''))

async function copyLink() {
  try {
    await navigator.clipboard.writeText(shareLink.value)
  } catch {
    return
  }
  copiedLink.value = true
  clearTimeout(linkTimer)
  linkTimer = setTimeout(() => {
    copiedLink.value = false
  }, 2_000)
}

let timer: ReturnType<typeof setInterval> | undefined

onMounted(() => {
  loadDiscounts()
  loadShop()
  timer = setInterval(refreshStatus, 5_000)
})

onBeforeUnmount(() => {
  if (timer) clearInterval(timer)
  clearTimeout(copiedTimer)
  clearTimeout(linkTimer)
})
</script>

<template>
  <UPageSection
    title="Create a payment QR"
    description="Enter what you want to charge, create the code, and show it to your customer."
  >
    <div class="grid gap-6 lg:grid-cols-2">
      <UCard>
        <template #header>
          <div class="flex items-center justify-between gap-3">
            <h2 class="text-base font-semibold">
              1. Invoice
            </h2>
            <WalletConnect />
          </div>
        </template>

        <div class="flex flex-col gap-4">
          <UFormField
            v-if="hasProducts"
            label="Price"
          >
            <URadioGroup
              v-model="priceMode"
              :items="priceModeItems"
            />
          </UFormField>

          <ProductPicker
            v-if="sellingFromShops"
            v-model="cart"
            :shops="shops"
          />

          <UFormField
            v-if="!sellingFromShops"
            label="Amount"
            :help="amountHelp"
          >
            <div class="flex items-stretch gap-2">
              <UInput
                v-if="priceInFiat"
                v-model="fiatAmount"
                type="number"
                min="0"
                step="0.01"
                class="min-w-0 flex-1"
                placeholder="5.00"
                :disabled="cartActive"
              />
              <UInput
                v-else
                v-model="amountSol"
                type="number"
                min="0"
                step="0.001"
                class="min-w-0 flex-1"
                placeholder="0.01"
                :disabled="cartActive"
              />
              <USelect
                v-model="amountUnit"
                :items="amountUnits"
                class="w-24"
                aria-label="Unit of the amount"
                :disabled="cartActive"
              />
              <UButton
                icon="i-lucide-calculator"
                color="neutral"
                variant="outline"
                aria-label="Open the calculator"
                :disabled="cartActive"
                @click="calculatorOpen = true"
              />
            </div>
          </UFormField>

          <UFormField
            label="Currency"
            :help="`The customer pays in ${currencySymbol(priceCurrency)} — one currency per code, fixed when you create it.`"
          >
            <div class="flex flex-wrap gap-2">
              <UButton
                v-for="choice in currencyChoices"
                :key="choice"
                :label="currencySymbol(choice)"
                size="sm"
                :color="priceCurrency === choice ? 'primary' : 'neutral'"
                :variant="priceCurrency === choice ? 'solid' : 'outline'"
                :aria-pressed="priceCurrency === choice"
                @click="priceCurrency = choice"
              />
            </div>
          </UFormField>

          <UFormField
            v-if="discounts.length"
            label="Discount"
            :help="selectedDiscount
              ? `Buyers pay ${discountedPrice} — ${selectedDiscount.percent_off}% off, applied for them.`
              : 'Optional — ride one of your codes along with this QR so buyers get it without typing it.'"
          >
            <USelect
              v-model="discountCode"
              :items="discountOptions"
              class="w-full"
            />
          </UFormField>

          <UFormField
            label="Name"
            help="The heading your customer sees above the amount — like “Coffee for two”."
          >
            <UInput
              v-model="name"
              class="w-full"
              placeholder="What is this payment?"
              :maxlength="64"
            />
          </UFormField>

          <UFormField
            label="Description"
            help="Optional detail under the name — what is included, where, when."
          >
            <UTextarea
              v-model="description"
              :rows="2"
              class="w-full"
              placeholder="Two flat whites at the corner shop"
              :maxlength="160"
            />
          </UFormField>

          <UFormField
            label="Memo"
            help="Recorded on chain with the payment, so wallets and explorers show it too."
          >
            <UTextarea
              v-model="memo"
              :rows="2"
              class="w-full"
              placeholder="Coffee"
              :maxlength="96"
            />
          </UFormField>

          <div class="flex items-center justify-between gap-3 text-sm text-muted">
            <span>{{ networkLabel(cluster) }}</span>
            <span class="tabular-nums">
              {{ priceSummary }}
            </span>
          </div>

          <UAlert
            v-if="failure"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            title="Could not create the payment"
            :description="failure"
          />

          <UButton
            label="Create payment code"
            icon="i-lucide-qr-code"
            :loading="busy"
            :disabled="!canCreate"
            block
            @click="createPayment"
          />
        </div>
      </UCard>

      <UCard>
        <template #header>
          <div class="flex items-center justify-between gap-3">
            <h2 class="text-base font-semibold">
              2. Show the code
            </h2>
            <UButton
              v-if="record"
              label="New payment"
              color="neutral"
              variant="soft"
              size="sm"
              @click="reset"
            />
          </div>
        </template>

        <div
          v-if="qrPayload && record"
          class="flex flex-col items-center gap-4"
        >
          <PaymentQr
            :payload="qrPayload"
            :size="320"
          />

          <p class="text-center text-xs text-muted">
            Any phone camera opens this — and the page takes the payment.
          </p>

          <div class="flex flex-wrap items-center justify-center gap-2">
            <UButton
              :to="shareLink"
              target="_blank"
              rel="noopener"
              label="Open payment page"
              icon="i-lucide-external-link"
            />
            <UButton
              :icon="copiedLink ? 'i-lucide-check' : 'i-lucide-link'"
              :label="copiedLink ? 'Link copied' : 'Copy link'"
              color="neutral"
              variant="soft"
              @click="copyLink"
            />
          </div>

          <div
            v-if="record.name || record.description"
            class="text-center"
          >
            <p
              v-if="record.name"
              class="text-base font-medium"
            >
              {{ record.name }}
            </p>
            <p
              v-if="record.description"
              class="mt-0.5 text-sm text-muted"
            >
              {{ record.description }}
            </p>
          </div>

          <!-- The receipt the customer sees, kept from the invoice itself: a
               status refresh replaces the record, not this. -->
          <div
            v-if="createdItems.length"
            class="w-full rounded-xl border border-default text-left"
          >
            <ul class="divide-y divide-default">
              <li
                v-for="(item, index) in createdItems"
                :key="index"
                class="flex items-baseline justify-between gap-3 px-3 py-1.5 text-sm"
              >
                <span class="min-w-0 break-words">{{ item.quantity }} × {{ item.name }}</span>
                <span class="shrink-0 text-muted tabular-nums">
                  {{ lamportsToSol(item.amount_lamports * item.quantity) }} SOL
                </span>
              </li>
            </ul>
          </div>

          <div class="flex items-center gap-2">
            <UBadge
              :color="statusColor(record.status)"
              variant="subtle"
              size="lg"
            >
              {{ statusLabel(record.status) }}
            </UBadge>
            <span class="text-sm text-muted">
              {{ paidAmountLabel(record) }}
              <template v-if="record.memo"> · {{ record.memo }}</template>
            </span>
            <UBadge
              v-if="record.discount"
              color="success"
              variant="subtle"
            >
              {{ record.discount.percent_off }}% off
            </UBadge>
            <UBadge
              v-if="paidCode && record.amount_lamports > 0"
              color="success"
              variant="subtle"
            >
              {{ paidCode }} · saved {{ lamportsToSol(priceLamports - record.amount_lamports) }} SOL
            </UBadge>
          </div>

          <dl class="w-full space-y-1 text-xs text-muted">
            <div class="flex justify-between gap-3">
              <dt>Payment ID</dt>
              <dd class="truncate font-mono">
                {{ record.payment_pda }}
              </dd>
            </div>
            <div
              v-if="record.settled_by"
              class="flex justify-between gap-3"
            >
              <dt>Paid by</dt>
              <dd class="truncate font-mono">
                {{ record.settled_by }}
              </dd>
            </div>
          </dl>

          <!--
            The same text the pay page accepts pasted in, so a customer who
            cannot scan the QR can still pay. `break-all` matters: the payload
            is one unbroken JSON string, and without it the card would grow to
            the payload's width instead of wrapping inside the card.
          -->
          <div ref="payloadBox">
            <div class="mb-1.5 flex items-center justify-between gap-3">
              <span class="text-sm font-medium">Payment code</span>
              <UButton
                :icon="copiedCode ? 'i-lucide-check' : 'i-lucide-copy'"
                :label="copiedCode ? 'Copied' : 'Copy code'"
                color="neutral"
                variant="soft"
                size="xs"
                @click="copyPayload"
              />
            </div>
            <code class="block w-full rounded-md border border-default bg-elevated/40 px-2.5 py-2 font-mono text-xs break-all whitespace-pre-wrap">
              {{ qrPayload }}
            </code>
            <p class="mt-1.5 text-xs text-muted">
              If the QR cannot be scanned, your customer can paste this on the pay page.
            </p>
          </div>
        </div>

        <div
          v-else
          class="py-10 text-center text-sm text-muted"
        >
          Your QR code will appear here once the payment is created.
        </div>
      </UCard>

      <UCard class="lg:col-span-2">
        <template #header>
          <div class="flex items-center justify-between gap-3">
            <h2 class="text-base font-semibold">
              3. Discount codes
            </h2>
            <span class="text-sm text-muted">
              Optional — buyers type a code at checkout to pay less.
            </span>
          </div>
        </template>

        <div class="flex flex-col gap-4">
          <ul
            v-if="discounts.length"
            class="flex flex-wrap gap-2"
          >
            <li
              v-for="discount in discounts"
              :key="discount.id"
              class="flex items-center gap-2 rounded-full border border-default py-1 pl-3 pr-1"
            >
              <span class="text-sm font-medium">{{ discount.code }}</span>
              <UBadge
                color="primary"
                variant="subtle"
              >
                {{ discount.percent_off }}% off
              </UBadge>
              <UButton
                icon="i-lucide-x"
                color="error"
                variant="ghost"
                size="xs"
                square
                :aria-label="`Delete ${discount.code}`"
                @click="removeDiscount(discount)"
              />
            </li>
          </ul>
          <p
            v-else
            class="text-sm text-muted"
          >
            No codes yet — add one so buyers can get a discount.
          </p>

          <div class="flex flex-wrap items-end gap-3">
            <UFormField
              label="Code"
              class="min-w-40 flex-1"
            >
              <UInput
                v-model="newCode"
                class="w-full"
                placeholder="SAVE10"
                :maxlength="32"
              />
            </UFormField>
            <UFormField
              label="Percent off"
              class="w-32"
            >
              <UInput
                v-model="newPercent"
                type="number"
                min="1"
                max="99"
                class="w-full"
                placeholder="10"
              />
            </UFormField>
            <UButton
              label="Add code"
              icon="i-lucide-plus"
              :loading="codeBusy"
              :disabled="!canAddCode"
              @click="addDiscount"
            />
          </div>

          <UAlert
            v-if="codeFailure"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            title="Discount code error"
            :description="codeFailure"
          />
        </div>
      </UCard>
    </div>

    <CalculatorModal
      v-model:open="calculatorOpen"
      :initial="priceInFiat ? fiatAmount : amountSol"
      @apply="applyAmount"
    />
  </UPageSection>
</template>
