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
import type { CartLine, ShopWithProducts } from '~/composables/useShop'
import { cartTotalLamports } from '~/composables/useShop'

const { t } = useI18n()

// The code is drawn at 320 and PaymentQr pads it by p-3 (0.75rem) a side, so
// the piece of paper the seller holds up is 344 wide. The button that presents
// it takes that width rather than the card's, so the two line up.
const CODE_SIZE = 320
const codePaperWidth = `${CODE_SIZE + 24}px`

useSeoMeta({ title: t('sell.metaTitle') })

definePageMeta({ middleware: 'auth' })

const { cluster } = useRuntimeConfig().public
const api = usePaymentApi()
const shopApi = useShopApi()
const { playSuccess } = useTransactionSounds()
// Pricing from a menu is only offered to sellers who turned shops on in
// settings — everyone else creates codes the way they always have.
const { shopsEnabled } = useShopPreference()
const { publicKey, connected } = useSolanaWallet()

// One price, in the currency the code charges: the dropdown beside the field is
// that currency, so there is no second control to reconcile against it. The
// seller types what they want to be paid and the customer hands over exactly it.
const priceInput = ref('0.01')
const { currency } = useCurrencyPreference()
// The one currency this invoice charges in. Offering several prices at once
// only hands the customer a decision; the seller decides, and the code says it.
const priceCurrency = ref<SupportedCurrency>('sol')
const currencyChoices = computed(() => currenciesForCluster(cluster))
const currencyItems = computed(() => currencyChoices.value.map(choice => ({
  label: currencySymbol(choice),
  value: choice
})))
// The amount field opens a calculator: sellers often price from arithmetic
// rather than a number they already know.
const calculatorOpen = ref(false)
const name = ref('')
const description = ref('')
const memo = ref('')
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
// The rest of the form is optional, so it stays folded: a seller who just wants
// to charge a number sees a number field, a name and one button. Each toggle is
// the same show/hide button the profile uses, rather than an accordion widget.
const detailsOpen = ref(false)
const discountsOpen = ref(false)
const codeOpen = ref(false)
// What a seller holds up to the customer's phone: the code, the name, the
// description and the price on their own screen, without the form around them.
const presentOpen = ref(false)
const priceModeItems = computed(() => [
  { label: t('sell.priceModes.amount'), value: 'amount' },
  { label: t('sell.priceModes.products'), value: 'products' }
])
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
  { label: t('sell.discountNone'), value: NONE },
  ...discounts.value.map(code => ({
    label: t('sell.discountOption', { code: code.code, percent: code.percent_off }),
    value: code.code
  }))
])

/** "1 code" / "2 codes", in whatever forms the locale defines. */
function codeCount(count: number) {
  return t('sell.codesCount', { count }, count)
}

const canAddCode = computed(() =>
  !codeBusy.value
  && newCode.value.trim().length >= 3
  && Number(newPercent.value) >= 1
  && Number(newPercent.value) <= 99
)

/**
 * The typed price in what the code charges: lamports for SOL, the token's own
 * base units for a stablecoin. A shop's cart sets it too — its products are
 * priced in lamports, which is why picking from a menu pins the currency to SOL.
 */
const priceUnits = computed(() => {
  if (sellingFromShops.value) return cartTotal.value
  const typed = Number(priceInput.value)
  if (!Number.isFinite(typed) || typed <= 0) return 0
  return priceCurrency.value === 'sol'
    ? solToLamports(typed)
    : Math.round(typed * 1_000_000)
})

/** The invoice's price in its own base units — exactly what the code charges. */
const invoiceOptions = computed(() => [
  { currency: priceCurrency.value, amount: priceUnits.value }
])

/** A price in the money the customer will hand over. */
function priceLabel(option: { currency: SupportedCurrency, amount: number }): string {
  return option.currency === 'sol'
    ? `${lamportsToSol(option.amount)} SOL`
    : `${formatTokenAmount(option.amount, 6)} ${currencySymbol(option.currency)}`
}

/** The price shown next to the form, and the same price after a carried code. */
const priceSummary = computed(() => priceLabel(invoiceOptions.value[0]!))

/** What a carried discount code took off, in the currency this code charges. */
const savedLabel = computed(() => {
  const currency = record.value?.options?.[0]?.currency ?? priceCurrency.value
  const saved = priceLamports.value - (record.value?.amount_lamports ?? 0)
  return saved > 0 ? priceLabel({ currency, amount: saved }) : ''
})
const discountedPrice = computed(() => {
  const option = invoiceOptions.value[0]!
  if (!selectedDiscount.value) return priceLabel(option)
  return priceLabel({
    currency: option.currency,
    amount: discountedLamports(option.amount, selectedDiscount.value.percent_off)
  })
})

/** The price field sets the invoice, and the dropdown says which currency it charges. */
const amountHelp = computed(() => {
  if (cartActive.value) return t('sell.amountHelpCart')
  if (sellingFromShops.value) return t('sell.amountHelpProducts')
  return t('sell.amountHelpTyped', { currency: currencySymbol(priceCurrency.value) })
})

// Picking products decides the price: the invoice has to charge exactly what
// the receipt adds up to, or the API refuses to build the code.
watch(cartTotal, (total) => {
  if (total <= 0) return
  priceCurrency.value = 'sol'
  priceInput.value = String(Number((total / 1_000_000_000).toFixed(9)))
})

// Switching back to a typed amount drops the tray: a cart left behind would
// price the invoice from items nobody can see any more.
watch(priceMode, (mode) => {
  if (mode === 'amount') cart.value = []
})

const canCreate = computed(() =>
  connected.value
  && !busy.value
  && priceUnits.value > 0
  && invoiceOptions.value.every(option => option.amount > 0)
)

const statusColor = (status: string) =>
  status === 'settled' ? 'success' : status === 'missing' || status === 'pending' ? 'warning' : 'neutral'

function applyAmount(amount: string) {
  priceInput.value = amount
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
    failure.value = t('sell.connectFirst')
    return
  }
  if (!Number.isFinite(priceUnits.value) || priceUnits.value <= 0) {
    failure.value = t('sell.enterPrice')
    return
  }
  if (memo.value.length > 96) {
    failure.value = t('sell.memoTooLong')
    return
  }
  if (name.value.trim().length > 64) {
    failure.value = t('sell.nameTooLong')
    return
  }
  if (description.value.trim().length > 160) {
    failure.value = t('sell.descriptionTooLong')
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
    priceLamports.value = priceUnits.value
    paidCode.value = null
    createdItems.value = created.payment.items ?? []
    // The invoice is made; the next one starts from an empty tray.
    cart.value = []
  } catch (error) {
    failure.value = error instanceof Error ? error.message : t('sell.createFailure')
  } finally {
    busy.value = false
  }
}

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
      // The seller is holding this screen up for the customer, so the payment
      // arriving is worth hearing without having to watch the badge.
      playSuccess()
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
    codeFailure.value = apiErrorMessage(error, t('sell.addDiscountFailure'))
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
    codeFailure.value = apiErrorMessage(error, t('sell.deleteDiscountFailure'))
  }
}

function reset() {
  record.value = null
  qrPayload.value = ''
  presentOpen.value = false
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
    :title="$t('sell.pageTitle')"
    :description="$t('sell.pageDescription')"
  >
    <div class="grid gap-6 lg:grid-cols-2">
      <UCard>
        <template #header>
          <div class="flex items-center justify-between gap-3">
            <h2 class="text-base font-semibold">
              {{ $t('sell.invoiceHeading') }}
            </h2>
            <WalletConnect />
          </div>
        </template>

        <div class="flex flex-col gap-4">
          <UFormField
            v-if="shopsEnabled && hasProducts"
            :label="$t('sell.pricingLabel')"
          >
            <URadioGroup
              v-model="priceMode"
              :items="priceModeItems"
            />
          </UFormField>

          <ProductPicker
            v-if="shopsEnabled && sellingFromShops"
            v-model="cart"
            :shops="shops"
          />

          <!--
            One field for the price and the currency it is charged in: the
            dropdown is not a display unit, it decides what the code charges.
          -->
          <UFormField
            v-if="!sellingFromShops"
            :label="$t('sell.priceLabel')"
            :help="amountHelp"
          >
            <div class="flex items-stretch gap-2">
              <UInput
                v-model="priceInput"
                type="number"
                min="0"
                :step="priceCurrency === 'sol' ? '0.001' : '0.01'"
                class="min-w-0 flex-1"
                :placeholder="priceCurrency === 'sol' ? $t('sell.pricePlaceholderSol') : $t('sell.pricePlaceholderToken')"
                :disabled="cartActive"
              />
              <USelect
                v-model="priceCurrency"
                :items="currencyItems"
                class="w-24"
                :aria-label="$t('sell.currencyAria')"
                :disabled="cartActive"
              />
              <UButton
                icon="i-lucide-calculator"
                color="neutral"
                variant="outline"
                :aria-label="$t('sell.calculatorAria')"
                :disabled="cartActive"
                @click="calculatorOpen = true"
              />
            </div>
          </UFormField>

          <UFormField
            v-if="discounts.length"
            :label="$t('sell.discountLabel')"
            :help="selectedDiscount
              ? $t('sell.discountHelpSelected', { price: discountedPrice, percent: selectedDiscount.percent_off })
              : $t('sell.discountHelp')"
          >
            <USelect
              v-model="discountCode"
              :items="discountOptions"
              class="w-full"
            />
          </UFormField>

          <UFormField
            :label="$t('sell.nameLabel')"
            :help="$t('sell.nameHelp')"
          >
            <UInput
              v-model="name"
              class="w-full"
              :placeholder="$t('sell.namePlaceholder')"
              :maxlength="64"
            />
          </UFormField>

          <!-- Two optional text fields: the buyer's detail line and the on-chain
               note. Folded, because most codes need neither. -->
          <div class="flex flex-col gap-3">
            <UButton
              :label="detailsOpen ? $t('sell.hideDetails') : $t('sell.addDetails')"
              :icon="detailsOpen ? 'i-lucide-chevron-up' : 'i-lucide-chevron-down'"
              color="neutral"
              variant="ghost"
              size="xs"
              block
              :aria-expanded="detailsOpen"
              @click="detailsOpen = !detailsOpen"
            />

            <template v-if="detailsOpen">
              <UFormField
                :label="$t('sell.descriptionLabel')"
                :help="$t('sell.descriptionHelp')"
              >
                <UTextarea
                  v-model="description"
                  :rows="2"
                  class="w-full"
                  :placeholder="$t('sell.descriptionPlaceholder')"
                  :maxlength="160"
                />
              </UFormField>

              <UFormField
                :label="$t('sell.memoLabel')"
                :help="$t('sell.memoHelp')"
              >
                <UTextarea
                  v-model="memo"
                  :rows="2"
                  class="w-full"
                  :placeholder="$t('sell.memoPlaceholder')"
                  :maxlength="96"
                />
              </UFormField>
            </template>
          </div>

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
            :title="$t('sell.createFailureTitle')"
            :description="failure"
          />

          <UButton
            :label="$t('sell.createCode')"
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
              {{ $t('sell.showHeading') }}
            </h2>
            <UButton
              v-if="record"
              :label="$t('sell.newPayment')"
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
          <!-- The code and the way to present it share one width. -->
          <div
            class="flex w-full flex-col items-center gap-4"
            :style="{ maxWidth: codePaperWidth }"
          >
            <PaymentQr
              :payload="qrPayload"
              :size="CODE_SIZE"
            />

            <!--
              The seller is the one showing the code, usually on this phone, so
              the way to hand it over is here rather than on the payment page the
              customer would have to open first.
            -->
            <UButton
              :label="$t('sell.showToCustomer')"
              icon="i-lucide-maximize"
              block
              @click="presentOpen = true"
            />
          </div>

          <p class="text-center text-xs text-muted">
            {{ $t('sell.scanHint') }}
          </p>

          <div class="flex flex-wrap items-center justify-center gap-2">
            <UButton
              :to="shareLink"
              target="_blank"
              rel="noopener"
              :label="$t('sell.openPage')"
              icon="i-lucide-external-link"
            />
            <UButton
              :icon="copiedLink ? 'i-lucide-check' : 'i-lucide-link'"
              :label="copiedLink ? $t('sell.linkCopied') : $t('sell.copyLink')"
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
            class="w-full rounded-xl border border-default text-start"
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
              {{ $t('sell.percentOff', { percent: record.discount.percent_off }) }}
            </UBadge>
            <UBadge
              v-if="paidCode && record.amount_lamports > 0"
              color="success"
              variant="subtle"
            >
              {{ $t('sell.savedBadge', { code: paidCode, amount: savedLabel }) }}
            </UBadge>
          </div>

          <!--
            Fallbacks and identifiers: needed when a phone will not scan, or when
            a payment has to be looked up. Folded so the code is what the card
            shows, which is what the seller actually holds up.
          -->
          <UButton
            v-if="!codeOpen"
            :label="$t('sell.showCodeText')"
            icon="i-lucide-chevron-down"
            color="neutral"
            variant="ghost"
            size="xs"
            :aria-expanded="false"
            @click="codeOpen = true"
          />

          <template v-else>
            <dl class="w-full space-y-1 text-xs text-muted">
              <div class="flex justify-between gap-3">
                <dt>{{ $t('sell.paymentId') }}</dt>
                <dd class="truncate font-mono">
                  {{ record.payment_pda }}
                </dd>
              </div>
              <div
                v-if="record.settled_by"
                class="flex justify-between gap-3"
              >
                <dt>{{ $t('sell.paidBy') }}</dt>
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
                <span class="text-sm font-medium">{{ $t('sell.paymentCode') }}</span>
                <UButton
                  :icon="copiedCode ? 'i-lucide-check' : 'i-lucide-copy'"
                  :label="copiedCode ? $t('common.copied') : $t('sell.copyCode')"
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
                {{ $t('sell.pasteHint') }}
              </p>
            </div>

            <UButton
              :label="$t('sell.hideDetailsButton')"
              icon="i-lucide-chevron-up"
              color="neutral"
              variant="ghost"
              size="xs"
              :aria-expanded="true"
              @click="codeOpen = false"
            />
          </template>
        </div>

        <div
          v-else
          class="py-10 text-center text-sm text-muted"
        >
          {{ $t('sell.emptyCode') }}
        </div>
      </UCard>

      <UCard class="lg:col-span-2">
        <template #header>
          <div class="flex items-center justify-between gap-3">
            <div>
              <h2 class="text-base font-semibold">
                {{ $t('sell.discountsTitle') }}
              </h2>
              <p class="text-sm text-muted">
                {{ $t('sell.discountsSubtitle') }}
              </p>
            </div>
            <UButton
              :label="discountsOpen
                ? $t('sell.hide')
                : discounts.length
                  ? codeCount(discounts.length)
                  : $t('sell.addCode')"
              :icon="discountsOpen ? 'i-lucide-chevron-up' : 'i-lucide-chevron-down'"
              color="neutral"
              variant="soft"
              size="sm"
              :aria-expanded="discountsOpen"
              @click="discountsOpen = !discountsOpen"
            />
          </div>
        </template>

        <div
          v-if="discountsOpen"
          class="flex flex-col gap-4"
        >
          <ul
            v-if="discounts.length"
            class="flex flex-wrap gap-2"
          >
            <li
              v-for="discount in discounts"
              :key="discount.id"
              class="flex items-center gap-2 rounded-full border border-default py-1 ps-3 pe-1"
            >
              <span class="text-sm font-medium">{{ discount.code }}</span>
              <UBadge
                color="primary"
                variant="subtle"
              >
                {{ $t('sell.percentOff', { percent: discount.percent_off }) }}
              </UBadge>
              <UButton
                icon="i-lucide-x"
                color="error"
                variant="ghost"
                size="xs"
                square
                :aria-label="$t('sell.deleteAria', { code: discount.code })"
                @click="removeDiscount(discount)"
              />
            </li>
          </ul>
          <p
            v-else
            class="text-sm text-muted"
          >
            {{ $t('sell.noCodes') }}
          </p>

          <div class="flex flex-wrap items-end gap-3">
            <UFormField
              :label="$t('sell.codeLabel')"
              class="min-w-40 flex-1"
            >
              <UInput
                v-model="newCode"
                class="w-full"
                :placeholder="$t('sell.codePlaceholder')"
                :maxlength="32"
              />
            </UFormField>
            <UFormField
              :label="$t('sell.percentLabel')"
              class="w-32"
            >
              <UInput
                v-model="newPercent"
                type="number"
                min="1"
                max="99"
                class="w-full"
                :placeholder="$t('sell.percentPlaceholder')"
              />
            </UFormField>
            <UButton
              :label="$t('sell.addCode')"
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
            :title="$t('sell.codeErrorTitle')"
            :description="codeFailure"
          />
        </div>
      </UCard>
    </div>

    <CalculatorModal
      v-model:open="calculatorOpen"
      :initial="priceInput"
      @apply="applyAmount"
    />

    <!--
      The screen a seller turns towards the customer: the code, the name, the
      description and the price, and nothing that belongs to the seller's side
      of the sale.
    -->
    <UModal
      v-if="record"
      v-model:open="presentOpen"
      :title="record.name || $t('sell.presentFallbackTitle')"
      :transition="false"
      :ui="{ content: 'max-w-md' }"
    >
      <template #body>
        <div class="flex flex-col items-center gap-4">
          <!-- 400 rather than the card's 320: an invoice link is long, so the
               code is dense, and it is read from further away here. It still
               shrinks to fit a narrow screen. -->
          <PaymentQr
            :payload="qrPayload"
            :size="400"
          />

          <div class="w-full text-center">
            <p
              v-if="record.description"
              class="text-sm text-muted"
            >
              {{ record.description }}
            </p>
            <p class="mt-2 text-2xl font-semibold tabular-nums">
              {{ paidAmountLabel(record) }}
            </p>
          </div>

          <p class="text-center text-xs text-muted">
            {{ $t('sell.scanToPay') }}
          </p>
        </div>
      </template>
    </UModal>
  </UPageSection>
</template>
