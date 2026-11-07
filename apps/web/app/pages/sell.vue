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
import type { CartLine, Product } from '~/composables/useShop'
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
// Which currencies this invoice takes. The seller prices the invoice once and
// every ticked currency gets its own price, pinned at creation time.
const accepted = ref<SupportedCurrency[]>(['sol'])
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

// The seller's own menu, offered as taps instead of typing. An account with no
// shop simply never sees this block.
const products = ref<Product[]>([])
const cart = ref<CartLine[]>([])
const cartTotal = computed(() => cartTotalLamports(cart.value))
const cartActive = computed(() => cart.value.length > 0)

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

const previewLamports = computed(() =>
  selectedDiscount.value
    ? discountedLamports(lamports.value, selectedDiscount.value.percent_off)
    : lamports.value
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

const lamports = computed(() =>
  priceInFiat.value
    ? lamportsFromFiat(Number(fiatAmount.value), rates.value, currency.value)
    : solToLamports(Number(amountSol.value))
)

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

/** Each accepted currency, priced in its own base units. */
const acceptedOptions = computed(() =>
  accepted.value.map((option) => {
    if (option === 'sol') return { currency: option, amount: lamports.value }
    const fiat = option === 'usdc' ? usdAmount.value : eurAmount.value
    return { currency: option, amount: Math.round(fiat * 1_000_000) }
  })
)

/** Naira-free summary of every price this invoice will charge. */
const priceSummary = computed(() =>
  acceptedOptions.value.map(option =>
    option.currency === 'sol'
      ? `${lamportsToSol(option.amount)} SOL`
      : `${formatTokenAmount(option.amount, 6)} ${currencySymbol(option.currency)}`
  )
)

// Picking products decides the price: the invoice has to charge exactly what
// the receipt adds up to, or the API refuses to build the code.
watch(cartTotal, (total) => {
  if (total <= 0) return
  priceInFiat.value = false
  amountSol.value = String(Number((total / 1_000_000_000).toFixed(9)))
})

function toggleAccepted(option: SupportedCurrency) {
  if (accepted.value.includes(option)) {
    // Never leave an invoice with no way to pay it.
    if (accepted.value.length > 1) accepted.value = accepted.value.filter(item => item !== option)
    return
  }
  accepted.value = currencyChoices.value.filter(item => item === option || accepted.value.includes(item))
}

const canCreate = computed(() =>
  connected.value
  && !busy.value
  && lamports.value > 0
  && acceptedOptions.value.every(option => option.amount > 0)
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

    // The API derives one invoice address per accepted currency from these
    // prices, so the code the customer scans carries every way to pay it.
    const created = await api.createPayment({
      seller,
      reference,
      options: acceptedOptions.value,
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
 * Every address this invoice can be settled on: one per accepted currency at
 * full price, plus one per currency per discount code. Whichever gets funded
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

/** The menu, if this account has one. A shop that fails to load must never
 *  stop a seller from typing an amount the way they always could. */
async function loadShop() {
  try {
    const response = await shopApi.get()
    products.value = response.products
  } catch {
    products.value = []
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
          <ProductPicker
            v-if="products.length"
            v-model="cart"
            :products="products"
          />

          <UFormField
            label="Amount"
            :help="cartActive
              ? 'The total of the products above — clear them to type an amount instead.'
              : priceInFiat
                ? 'Charged in SOL at today\'s rate — the invoice keeps the amount below.'
                : 'What the customer pays.'"
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
            label="Accept payment in"
            help="Tick every currency you take — the customer picks one on their own screen."
          >
            <div class="flex flex-wrap gap-2">
              <UButton
                v-for="choice in currencyChoices"
                :key="choice"
                :label="currencySymbol(choice)"
                size="sm"
                :color="accepted.includes(choice) ? 'primary' : 'neutral'"
                :variant="accepted.includes(choice) ? 'solid' : 'outline'"
                @click="toggleAccepted(choice)"
              />
            </div>
          </UFormField>

          <UFormField
            v-if="discounts.length"
            label="Discount"
            :help="selectedDiscount
              ? `Buyers pay ${lamportsToSol(previewLamports)} SOL — ${selectedDiscount.percent_off}% off, applied for them.`
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
              {{ priceSummary.join(' · ') }}
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
