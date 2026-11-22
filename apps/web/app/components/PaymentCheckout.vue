<script setup lang="ts">
import type { Shop } from '~/composables/useShop'

const props = withDefaults(
  defineProps<{
    source?: string
    /**
     * A shared or printed link outlives the visit, so it is worth keeping and
     * printing. A code read at the counter is paid on the spot and has nothing
     * to save, so the scan flow turns this off.
     */
    permanent?: boolean
  }>(),
  { permanent: true }
)

// The shared link is the checkout: one sheet with what is being sold, its
// price, and the single action that pays it. It still has to survive being
// printed, so paper keeps the details and the code and drops every button.
//
// Which action that is depends on the buyer, not on the invoice: a browser
// that can settle it from a monthly allowance pays with no wallet at all, a
// connected wallet asks for one approval, and a browser with no wallet gets
// the list of wallets (on iOS those are deep links into the wallet app).

const { t } = useI18n()
const api = usePaymentApi()
const { user, ensureSession } = useAuth()
const { wallets, selectedWallet, selectWallet } = useSolanaWallets()
const { connect, connecting, canConnect, connected } = useSolanaWallet()

const {
  payload,
  code,
  record,
  error,
  failure,
  busy,
  signature,
  discount,
  discountInput,
  discountBusy,
  discountError,
  budgetState,
  budgetObstacle,
  holding,
  holdProgress,
  payableLamports,
  payableAmount,
  priceAmount,
  costs,
  costsError,
  options,
  selected,
  selectCurrency,
  offersChoice,
  settled,
  paymentName,
  paymentDescription,
  paymentMemo,
  items,
  canPayFromAllowance,
  canPayWithWallet,
  blockedReason,
  treasuryTokenAccountMissing,
  stage,
  load,
  pay,
  applyDiscount,
  clearDiscount,
  beginHold,
  stopHold
} = usePaymentCheckout()

const heading = computed(() => paymentName.value || paymentMemo.value || t('checkout.fallbackHeading'))

const discountOpen = ref(false)

// The result of this visit's payment, announced over the sheet: a settled
// invoice or a refusal, with the code the buyer can read back to the seller.
const resultOpen = ref(false)
const resultKind = ref<'success' | 'error'>('success')
// The address that was actually paid, not the invoice's own id: an invoice
// that takes several currencies has one address per currency, and the seller's
// transaction list is keyed by the one the money went to.
const refCode = computed(() => {
  const paid = record.value?.payment_pda || payload.value?.id
  return paid ? referenceCode(paid) : ''
})
const sellerShort = computed(() => {
  const seller = payload.value?.seller ?? ''
  return seller.length > 12 ? `${seller.slice(0, 4)}…${seller.slice(-4)}` : seller
})
const { playSuccess, playFailure } = useTransactionSounds()

// Only a payment made here is announced. A link that opens already paid says
// so on the sheet; nobody was waiting for a sound by then.
watch(settled, (isSettled) => {
  if (!isSettled || !signature.value) return
  resultKind.value = 'success'
  resultOpen.value = true
  playSuccess()
})

watch(failure, (message) => {
  if (!message) return
  resultKind.value = 'error'
  resultOpen.value = true
  playFailure()
})

const successLine = computed(() =>
  sellerShort.value
    ? t('checkout.result.sentLine', { amount: payableLabel.value, seller: sellerShort.value })
    : payableLabel.value
)

const copiedRef = ref(false)

async function copyRefCode() {
  try {
    await navigator.clipboard.writeText(refCode.value)
  } catch {
    return
  }
  copiedRef.value = true
  setTimeout(() => {
    copiedRef.value = false
  }, 2_000)
}

/**
 * The signature proves the payment in both stages — while the receipt catches up
 * and once it has — and in the dialog a buyer may leave on screen after closing
 * the sheet, so what the line says is written once here rather than three times
 * in the template, and none of them can drift from the others.
 */
const signatureProof = computed(() => ({
  signature: signature.value ?? '',
  cluster: payload.value?.cluster ?? '',
  label: t('checkout.settled.signatureLabel'),
  copyLabel: t('checkout.settled.copySignature'),
  copiedLabel: t('common.copied'),
  explorerLabel: t('checkout.settled.explorer')
}))

/**
 * Written once and printed on the sheet: paper has nothing to click, so a printed
 * sheet carries the same facts the screen shows. That includes the code the buyer
 * reads back and, once it is paid, the wallet that paid it — the copy row and its
 * explorer link live in the part of the page the print view hides, so without
 * these rows the paper would describe a payment it cannot be traced from.
 */
interface DetailRow {
  label: string
  value: string
  mono: boolean
  /** A code or an address: it wraps on paper instead of being cut short. */
  wrap?: boolean
}

const detailRows = computed<DetailRow[]>(() => [
  { label: t('checkout.details.seller'), value: String(payload.value?.seller ?? ''), mono: true, wrap: true },
  { label: t('checkout.details.network'), value: networkLabel(payload.value?.cluster || ''), mono: false },
  ...(record.value
    ? [{ label: t('checkout.details.status'), value: statusLabel(record.value.status), mono: false }]
    : []),
  ...(refCode.value
    ? [{ label: t('checkout.referenceCode'), value: refCode.value, mono: true, wrap: true }]
    : []),
  ...(record.value?.settled_by
    ? [{ label: t('checkout.settled.paidByLabel'), value: record.value.settled_by, mono: true, wrap: true }]
    : [])
])

// The invoice is charged in SOL. The figure in brackets is what that price is
// worth in the seller's currency, which travels inside the code — the buyer
// picks nothing. Older codes carry no currency, so the reader's own default
// stands in for it.
const { currency } = useCurrencyPreference()
const rates = useSolRates()
const priceCurrency = computed<FiatCurrency>(() => payload.value?.currency ?? currency.value)

/** The amount as it is charged, in the currency the buyer picked. */
const payableNumber = computed(() =>
  formatTokenAmount(payableAmount.value, selected.value.decimals)
)
const payableLabel = computed(() =>
  `${payableNumber.value} ${currencySymbol(selected.value.currency)}`
)
const priceLabel = computed(() =>
  `${formatTokenAmount(priceAmount.value, selected.value.decimals)} ${currencySymbol(selected.value.currency)}`
)

/**
 * What the charge is worth in fiat. Only SOL has a price to convert: a
 * stablecoin is already denominated in dollars or euros, so an estimate beside
 * "5 USDC" would just repeat the same number in brackets.
 */
const fiatPrice = computed(() => {
  if (selected.value.currency !== 'sol') return null
  return fiatFromLamports(payableLamports.value, rates.value, priceCurrency.value)
})

/**
 * The receipt, priced in whatever the buyer is paying with. Each line's share
 * of the invoice total scales with the total, so switching from SOL to USDC or
 * applying a code moves every line with the amount rather than contradicting it.
 */
const receiptLines = computed(() => {
  const total = items.value.reduce(
    (sum, item) => sum + item.amount_lamports * item.quantity,
    0
  )
  const share = total ? payableAmount.value / total : 1
  return items.value.map(item => ({
    name: item.name,
    quantity: item.quantity,
    amount: `${formatTokenAmount(
      Math.round(item.amount_lamports * item.quantity * share),
      selected.value.decimals
    )} ${currencySymbol(selected.value.currency)}`
  }))
})

// Keeping the code files it under one of the account's shops, so the shop page
// can list the links it hands out. Shops are an account's own catalogue and most
// accounts never turn them on, so the whole offer stays out of sight for them —
// a buyer who happens to be signed in should not be sold a shop while paying.
const { shopsEnabled } = useShopPreference()
const shopApi = useShopApi()
const shops = ref<Shop[]>([])
const shopId = ref('')
const saved = ref(false)
const savedShopId = ref('')
const saving = ref(false)
const saveError = ref('')

const shopChoices = computed(() => shops.value.map(shop => ({ label: shop.name, value: shop.id })))
const savedShop = computed(() => shops.value.find(shop => shop.id === savedShopId.value))

/**
 * Printing is the seller's own action: they are the one putting the code on a
 * wall. A buyer paying a one-time invoice has nothing to print, so the button
 * only shows to the account that made the code.
 */
const isMyCode = computed(
  () => Boolean(user.value && payload.value?.seller_user && user.value.id === payload.value.seller_user)
)

/**
 * The code a Solana wallet understands, offered as a link for someone whose
 * wallet registers the `solana:` scheme (Phantom and Solflare both do). The
 * page stays the default: this is the escape hatch, not the main path.
 */
const config = useRuntimeConfig()
const origin = useRequestURL().origin
const walletLink = computed(() => {
  if (!payload.value) return ''
  const apiBase = (config.public.apiBase as string) || origin
  return solanaPayUrl(JSON.stringify(payload.value), apiBase)
})

// Only shown where a wallet app can actually take it over; on a desktop this
// link would land on a browser error page.
const handheld = ref(false)

/** Wallets that open their own app (iOS/Android) rather than inject a provider. */
function opensApp(wallet: { source?: string, platform?: string }) {
  return wallet.platform === 'mobile' || wallet.source === 'deep-link'
}

function walletLabel(wallet: { name: string, source?: string, platform?: string }) {
  return opensApp(wallet)
    ? t('checkout.wallet.openIn', { wallet: wallet.name })
    : t('checkout.wallet.connect', { wallet: wallet.name })
}

async function pick(wallet: NonNullable<typeof selectedWallet.value>) {
  selectWallet(wallet)
  await nextTick()
  if (canConnect.value && !connected.value) await connect()
}

/** Opening the code field is a decision to price this differently first. */
function openDiscount() {
  stopHold()
  discountOpen.value = true
}

async function refreshSaved() {
  saved.value = false
  savedShopId.value = ''
  const current = payload.value
  if (!user.value || !current || !shopsEnabled.value) return
  try {
    const [kept, menus] = await Promise.all([api.listSavedPayments(), shopApi.list()])
    shops.value = menus
    shopId.value = menus[0]?.id ?? ''
    const entry = kept.find(item => item.payment_id === current.id)
    if (entry) {
      saved.value = true
      savedShopId.value = entry.shop_id ?? ''
    }
  } catch {
    // Not worth an error of its own: the save action reports its own failures.
  }
}

async function saveToShop() {
  const current = payload.value
  if (!current || !shopId.value) return
  saving.value = true
  saveError.value = ''
  try {
    const entry = await api.savePayment(JSON.stringify(current), shopId.value)
    saved.value = true
    savedShopId.value = entry.shop_id ?? shopId.value
  } catch (cause) {
    saveError.value = apiErrorMessage(cause, t('checkout.save.failure'))
  } finally {
    saving.value = false
  }
}

/**
 * The print dialog is the only Save-as-PDF the browsers agree on, and it keeps
 * the code as vectors — a rasterised PDF would scan badly once printed at
 * receipt size. The page is styled for paper, so this just opens it.
 *
 * Only the seller's own code reaches this: the button beside it is behind
 * `isMyCode`, and a buyer has nothing to fix to a wall.
 */
function saveAsPdf() {
  window.print()
}

onMounted(async () => {
  handheld.value = /Android|iPhone|iPad|iPod/i.test(navigator.userAgent)
  if (props.source) await load(props.source)

  // Anyone can pay; only a signed-in visitor can keep a copy.
  await ensureSession()
  await refreshSaved()
})

// The same page can be handed a different invoice by a link inside it.
watch(() => props.source, (next) => {
  if (next) void load(next)
})

// The suggested PDF filename is the page title, so the name leads it.
useSeoMeta({
  title: () => (paymentName.value
    ? t('checkout.metaTitleWithName', { name: paymentName.value })
    : t('checkout.metaTitle'))
})
</script>

<template>
  <!--
    On paper the sheet is the page itself: the card is placed against the
    page box and the brand line against its bottom edge. Sizing anything to
    the viewport would instead be sized to the paper, and a single pixel of
    overhang is enough for the browser to spill a second, empty page.
  -->
  <div class="flex min-h-[100svh] flex-col print:block print:min-h-0 print:bg-default">
    <main class="flex flex-1 items-center justify-center px-5 py-10 print:fixed print:inset-0 print:flex print:items-center print:justify-center print:bg-default print:p-[12mm]">
      <div class="w-full max-w-sm">
        <section
          class="rounded-2xl border border-default bg-default p-6 shadow-sm print:break-inside-avoid print:bg-elevated"
        >
          <div
            v-if="stage === 'loading'"
            class="flex flex-col items-center gap-4 py-8"
          >
            <div class="aspect-square w-full max-w-[240px] animate-pulse rounded-xl bg-elevated/50" />
            <p class="text-sm text-muted">
              {{ $t('checkout.loading') }}
            </p>
          </div>

          <div
            v-else-if="stage === 'invalid'"
            class="flex flex-col items-center gap-2 py-6 text-center"
          >
            <UIcon
              name="i-lucide-unlink"
              class="size-8 text-muted"
            />
            <h1 class="text-base font-semibold">
              {{ $t('checkout.invalidTitle') }}
            </h1>
            <p class="text-sm text-muted">
              {{ error }}
            </p>
            <NuxtLink
              to="/"
              class="mt-2 text-sm font-medium text-primary underline-offset-4 hover:underline"
            >
              {{ $t('checkout.goHome') }}
            </NuxtLink>
          </div>

          <div
            v-else
            class="flex flex-col items-center text-center"
          >
            <h1 class="text-xl font-semibold">
              {{ heading }}
            </h1>
            <p
              v-if="paymentDescription"
              class="mt-1 text-sm text-muted"
            >
              {{ paymentDescription }}
            </p>

            <!-- What the money is for, line by line. A receipt the customer can
                 check before paying beats a single total they have to trust. -->
            <div
              v-if="receiptLines.length"
              class="mt-4 w-full max-w-sm rounded-xl border border-default text-start"
            >
              <ul class="divide-y divide-default">
                <li
                  v-for="(line, index) in receiptLines"
                  :key="index"
                  class="flex items-baseline justify-between gap-3 px-3 py-2 text-sm"
                >
                  <span class="min-w-0 break-words">
                    <span class="text-muted">{{ line.quantity }} ×</span> {{ line.name }}
                  </span>
                  <span class="shrink-0 text-muted tabular-nums">{{ line.amount }}</span>
                </li>
              </ul>
              <div class="flex items-baseline justify-between gap-3 border-t border-default px-3 py-2 text-sm font-medium">
                <span>{{ $t('checkout.total') }}</span>
                <span class="tabular-nums">{{ payableLabel }}</span>
              </div>
            </div>

            <!-- The seller decides which currencies this invoice takes; the
                 buyer only chooses among them. -->
            <div
              v-if="offersChoice"
              class="mt-4 flex flex-wrap items-center justify-center gap-2 print:hidden"
            >
              <UButton
                v-for="option in options"
                :key="option.currency"
                :label="currencySymbol(option.currency)"
                size="sm"
                :color="selected.currency === option.currency ? 'primary' : 'neutral'"
                :variant="selected.currency === option.currency ? 'solid' : 'outline'"
                @click="selectCurrency(option.currency)"
              />
            </div>

            <div class="mt-6 flex flex-col items-center gap-1">
              <UBadge
                v-if="discount"
                color="success"
                variant="subtle"
                size="sm"
              >
                {{ $t('checkout.discount.off', { percent: discount.percent_off }) }}
              </UBadge>
              <p class="flex flex-wrap items-baseline justify-center gap-x-2 text-4xl font-bold tabular-nums">
                {{ payableNumber }}
                <span class="text-xl font-semibold text-muted">{{ currencySymbol(selected.currency) }}</span>
                <span
                  v-if="fiatPrice"
                  class="text-base font-normal text-muted"
                >
                  (≈ {{ fiatPrice }})
                </span>
              </p>
              <p
                v-if="discount"
                class="text-sm text-muted tabular-nums"
              >
                <span class="line-through">{{ priceLabel }}</span>
              </p>

              <!--
                What the price does not include: the buyer's own Solana cost.
                Both settles take the fee out of the seller's side and refund
                the record rents from the treasury, so the one line left is the
                network fee the buyer pays to send the transaction.
              -->
              <dl
                v-if="costs"
                class="mt-2 flex w-full max-w-xs flex-col gap-0.5 text-xs text-muted tabular-nums"
              >
                <div
                  v-if="costs.payerPays > costs.amount"
                  class="flex items-baseline justify-between gap-4"
                >
                  <dt>{{ $t('checkout.protocolFee') }}</dt>
                  <dd>
                    ≈ {{ formatTokenAmount(costs.payerPays - costs.amount, selected.decimals) }}
                    {{ currencySymbol(selected.currency) }}
                  </dd>
                </div>
                <div class="flex items-baseline justify-between gap-4">
                  <dt>{{ $t('checkout.networkFee') }}</dt>
                  <dd>≈ {{ lamportsToSol(costs.networkFee) }} SOL</dd>
                </div>
              </dl>
              <!--
                A token invoice can only pay into an account the seller already
                opened: creating one inside someone else's settlement would hand
                them rent, so the page says so instead of quoting a fee for it.
              -->
              <p
                v-if="treasuryTokenAccountMissing"
                class="mt-1 max-w-xs text-xs text-warning"
              >
                {{ $t('checkout.treasuryTokenAccountMissing') }}
              </p>
              <p
                v-else-if="costs && costs.sellerTokenAccountRent > 0"
                class="mt-1 max-w-xs text-xs text-warning"
              >
                {{ $t('checkout.sellerTokenAccountMissing') }}
              </p>
              <p
                v-else-if="costsError"
                class="mt-2 max-w-xs text-xs text-warning"
              >
                {{ costsError }}
              </p>
            </div>

            <!--
              The one action, chosen by what this browser can do — never by
              what the seller would prefer it to do.
            -->
            <div class="mt-6 w-full print:hidden">
              <div
                v-if="stage === 'settled'"
                class="flex flex-col gap-3"
              >
                <UAlert
                  color="success"
                  variant="subtle"
                  icon="i-lucide-circle-check"
                  :title="$t('checkout.settled.title')"
                  :description="$t('checkout.settled.description')"
                />
                <!-- The code stays on the sheet too, not only in the result
                     dialog the buyer may have already dismissed. -->
                <!-- The code and the wallet that paid copy the same way the
                     signature does: shortened on the screen, whole on paper. -->
                <CopyField
                  v-if="refCode"
                  :label="$t('checkout.referenceCode')"
                  :value="refCode"
                  :copy-label="$t('checkout.result.copyCode')"
                  :copied-label="$t('common.copied')"
                />
                <SignatureProof
                  v-if="signature"
                  v-bind="signatureProof"
                />
                <CopyField
                  v-else-if="record?.settled_by"
                  :label="$t('checkout.settled.paidByLabel')"
                  :value="record.settled_by"
                  :keep="6"
                  :copy-label="$t('checkout.settled.copyWallet')"
                  :copied-label="$t('common.copied')"
                />
              </div>

              <!--
                The send is confirmed, but the record the page reads comes from
                the API and has not caught up with it. Another press in that
                window would only be rejected on chain, so the button stays away
                until the receipt lands.
              -->
              <div
                v-else-if="stage === 'confirming'"
                class="flex flex-col gap-2"
              >
                <div
                  class="flex items-center justify-center gap-2 rounded-xl border border-default px-4 py-3"
                >
                  <UIcon
                    name="i-lucide-loader-circle"
                    class="size-4 animate-spin"
                    aria-hidden="true"
                  />
                  <span class="text-sm text-muted">{{ $t('checkout.confirming') }}</span>
                </div>
                <!-- The chain already has this payment, so the signature is the
                     buyer's proof of it while the receipt is slow — the same
                     line the settled view keeps once the record arrives. -->
                <SignatureProof
                  v-if="signature"
                  v-bind="signatureProof"
                />
              </div>

              <!--
                Paying from the allowance sends without a wallet approval, so
                the press is the consent: the button has to be held for the
                whole count, and letting go sends and charges nothing.
              -->
              <template v-else-if="stage === 'silent'">
                <HoldToPayButton
                  :label="$t('checkout.hold.label', { amount: payableLabel })"
                  :holding-label="$t('checkout.hold.holding')"
                  :holding="holding"
                  :progress="holdProgress"
                  :loading="busy"
                  :disabled="!canPayFromAllowance"
                  :hint="$t('checkout.hold.hint')"
                  :blocked-reason="
                    treasuryTokenAccountMissing ? $t('checkout.hold.blocked') : ''
                  "
                  @start="beginHold"
                  @stop="stopHold"
                />
              </template>

              <!-- A connected wallet: one tap, straight to its approval. -->
              <template v-else-if="stage === 'wallet'">
                <UButton
                  :label="$t('checkout.pay', { amount: payableLabel })"
                  icon="i-lucide-wallet"
                  :loading="busy"
                  :disabled="!canPayWithWallet"
                  block
                  @click="pay()"
                />
                <p
                  v-if="blockedReason"
                  class="mt-2 text-xs text-warning"
                >
                  {{ blockedReason }}
                </p>
                <p
                  v-else
                  class="mt-2 text-xs text-muted"
                >
                  {{ $t('checkout.walletHint', { wallet: selectedWallet?.name || $t('checkout.yourWallet') }) }}
                </p>
                <UAlert
                  v-if="budgetObstacle"
                  color="warning"
                  variant="subtle"
                  icon="i-lucide-circle-alert"
                  :title="$t('checkout.obstacleTitle')"
                  :description="budgetObstacle"
                  class="mt-3 text-start"
                />
                <div
                  v-else-if="!budgetState"
                  class="mt-3 rounded-xl border border-dashed border-default p-3 text-start"
                >
                  <p class="text-xs font-medium">
                    {{ $t('checkout.allowanceOffer.title') }}
                  </p>
                  <p class="mt-1 text-xs text-muted">
                    {{ $t('checkout.allowanceOffer.description') }}
                  </p>
                  <UButton
                    to="/budget"
                    :label="$t('checkout.allowanceOffer.action')"
                    color="primary"
                    variant="soft"
                    size="xs"
                    class="mt-2"
                  />
                </div>
              </template>

              <!-- A budget exists, but this device cannot sign for it yet. -->
              <div
                v-else-if="stage === 'setup'"
                class="flex flex-col gap-2 text-start"
              >
                <UAlert
                  color="info"
                  variant="subtle"
                  icon="i-lucide-key-round"
                  :title="$t('checkout.deviceSetup.title')"
                  :description="$t('checkout.deviceSetup.description')"
                />
                <UButton
                  to="/budget"
                  :label="$t('checkout.deviceSetup.action')"
                  block
                />
                <UButton
                  :label="$t('checkout.payWith', { wallet: selectedWallet?.name || $t('checkout.yourWallet') })"
                  :loading="busy"
                  :disabled="!canPayWithWallet"
                  color="neutral"
                  variant="soft"
                  block
                  @click="pay()"
                />
                <p
                  v-if="blockedReason"
                  class="text-xs text-warning"
                >
                  {{ blockedReason }}
                </p>
              </div>

              <!-- Nothing connected: the wallets this browser can offer. -->
              <div
                v-else-if="stage === 'pick'"
                class="flex flex-col gap-2"
              >
                <p class="text-sm font-medium">
                  {{ $t('checkout.pick.title') }}
                </p>
                <UButton
                  v-for="wallet in wallets"
                  :key="wallet.name"
                  :label="walletLabel(wallet)"
                  color="primary"
                  variant="soft"
                  block
                  :loading="connecting && selectedWallet?.name === wallet.name"
                  @click="pick(wallet)"
                />
                <p
                  v-if="!wallets.length"
                  class="text-xs text-muted"
                >
                  {{ $t('checkout.pick.none') }}
                </p>
                <UButton
                  v-if="handheld && walletLink"
                  :to="walletLink"
                  :label="$t('checkout.pick.openInWallet')"
                  icon="i-lucide-external-link"
                  color="neutral"
                  variant="ghost"
                  size="sm"
                />
              </div>

              <UAlert
                v-if="failure"
                color="error"
                variant="subtle"
                icon="i-lucide-circle-alert"
                :title="$t('checkout.failureTitle')"
                :description="failure"
                class="mt-3 text-start"
              />
            </div>

            <!-- Priced by a code: one typed here, on an invoice whose seller
                 invited it. The flag is the seller's own answer, so the page
                 asks nothing else of the invoice. -->
            <div
              v-if="payload?.seller_user && payload?.allow_discounts && !discount && !settled"
              class="mt-3 w-full print:hidden"
            >
              <UButton
                v-if="!discountOpen"
                :label="$t('checkout.discount.haveCode')"
                color="neutral"
                variant="ghost"
                size="xs"
                @click="openDiscount"
              />
              <div
                v-else
                class="flex items-start gap-2"
              >
                <UInput
                  v-model="discountInput"
                  class="flex-1"
                  size="sm"
                  :placeholder="$t('checkout.discount.placeholder')"
                  :maxlength="32"
                  @focus="stopHold"
                />
                <UButton
                  :label="$t('checkout.discount.apply')"
                  size="sm"
                  :loading="discountBusy"
                  :disabled="discountInput.trim().length < 3"
                  @click="applyDiscount()"
                />
              </div>
              <UAlert
                v-if="discountError"
                color="error"
                variant="subtle"
                icon="i-lucide-circle-alert"
                :description="discountError"
                class="mt-2 text-start"
              />
            </div>

            <!-- The badge is the text and Remove is the action: on a phone the
                 action drops below it rather than squeezing onto the same row. -->
            <div
              v-if="discount && !settled"
              class="mt-2 flex flex-col items-center gap-2 print:hidden sm:flex-row sm:flex-wrap sm:justify-center"
            >
              <UBadge
                color="success"
                variant="subtle"
              >
                {{ $t('checkout.discount.badge', { code: discount.code, percent: discount.percent_off }) }}
              </UBadge>
              <UButton
                :label="$t('checkout.discount.remove')"
                color="neutral"
                variant="ghost"
                size="xs"
                @click="clearDiscount"
              />
            </div>

            <!-- Paper has nothing to click, so the code and the invoice's
                 identifiers are always printed. -->
            <div class="mt-6 hidden w-full print:block">
              <div class="mx-auto w-full max-w-[240px]">
                <PaymentQr
                  :payload="code"
                  :size="240"
                />
              </div>
              <dl class="mt-4 w-full space-y-1 text-xs text-muted">
                <div
                  v-for="row in detailRows"
                  :key="row.label"
                  class="flex justify-between gap-3"
                >
                  <dt class="shrink-0 whitespace-nowrap">
                    {{ row.label }}
                  </dt>
                  <dd
                    class="min-w-0 text-end"
                    :class="[row.mono ? 'font-mono' : '', row.wrap ? 'break-all' : 'truncate']"
                  >
                    {{ row.value }}
                  </dd>
                </div>
              </dl>
            </div>
          </div>
        </section>

        <!--
          An action belonging to whichever page opened the sheet — the
          scanner's "Scan another", say. Kept out of the row below so it still
          shows when no invoice could be read, which is when it matters most.
        -->
        <div
          v-if="$slots.actions && stage !== 'loading'"
          class="mt-3 flex flex-wrap items-center justify-center gap-2 print:hidden"
        >
          <slot name="actions" />
        </div>

        <div
          v-if="permanent && stage !== 'loading' && stage !== 'invalid'"
          class="mt-3 flex flex-wrap items-center justify-center gap-2 print:hidden"
        >
          <UButton
            v-if="saved"
            :to="savedShopId ? `/shops/${savedShopId}` : '/profile'"
            :label="savedShop ? $t('checkout.save.savedTo', { shop: savedShop.name }) : $t('checkout.save.savedToShop')"
            icon="i-lucide-bookmark-check"
            color="neutral"
            variant="ghost"
            size="sm"
          />
          <template v-else-if="user && shopsEnabled && shops.length">
            <USelect
              v-if="shops.length > 1"
              v-model="shopId"
              :items="shopChoices"
              class="w-40"
              size="sm"
              :aria-label="$t('checkout.save.shopAria')"
            />
            <UButton
              :label="$t('checkout.save.action')"
              icon="i-lucide-bookmark-plus"
              color="neutral"
              variant="ghost"
              size="sm"
              :loading="saving"
              @click="saveToShop"
            />
          </template>
          <UButton
            v-if="isMyCode"
            :label="$t('checkout.save.asPdf')"
            icon="i-lucide-file-down"
            color="neutral"
            variant="ghost"
            size="sm"
            @click="saveAsPdf"
          />
        </div>

        <UAlert
          v-if="saveError"
          color="error"
          variant="subtle"
          icon="i-lucide-circle-alert"
          :title="$t('checkout.save.failureTitle')"
          :description="saveError"
          class="mt-2"
        />
      </div>
    </main>

    <!--
      The outcome of the payment just made, in one screen. Unmounted on close
      so its entrance plays again if the buyer pays a second time.
    -->
    <UModal
      v-model:open="resultOpen"
      :transition="false"
      unmount-on-hide
      :ui="{ content: 'max-w-sm' }"
    >
      <template #body>
        <div
          v-if="resultKind === 'success'"
          class="flex flex-col items-center gap-3 text-center"
        >
          <div
            class="relative grid size-18 place-items-center rounded-full bg-success/10 text-success animate-pay-result-pop motion-reduce:animate-none"
          >
            <UIcon
              name="i-lucide-circle-check"
              class="size-12"
            />
            <span
              class="absolute inset-0 rounded-full border-2 border-current opacity-0 animate-pay-result-halo motion-reduce:hidden"
              aria-hidden="true"
            />
          </div>
          <p
            class="text-lg font-semibold animate-pay-rise-1 motion-reduce:animate-none"
          >
            {{ $t('checkout.result.sent') }}
          </p>
          <p
            class="text-sm text-muted animate-pay-rise-1 motion-reduce:animate-none"
          >
            {{ successLine }}
          </p>

          <div
            class="mt-1 w-full rounded-xl border border-default p-3 animate-pay-rise-2 motion-reduce:animate-none"
          >
            <p class="text-xs text-muted">
              {{ $t('checkout.referenceCode') }}
            </p>
            <p
              class="font-mono text-2xl font-semibold tracking-[0.2em] animate-pay-result-code motion-reduce:animate-none"
            >
              {{ refCode }}
            </p>
            <p class="mt-1 text-xs text-muted">
              {{ $t('checkout.result.readToSeller') }}
            </p>
          </div>

          <!-- The sheet's proof, repeated here: a buyer who closes the dialog and
               keeps only this screen must still be able to open the payment. -->
          <SignatureProof
            v-if="signature"
            v-bind="signatureProof"
            class="w-full animate-pay-rise-3 motion-reduce:animate-none"
          />

          <div
            class="mt-1 flex w-full gap-2 animate-pay-rise-3 motion-reduce:animate-none"
          >
            <UButton
              :icon="copiedRef ? 'i-lucide-check' : 'i-lucide-copy'"
              :label="copiedRef ? $t('common.copied') : $t('checkout.result.copyCode')"
              color="neutral"
              variant="soft"
              class="flex-1 justify-center"
              @click="copyRefCode"
            />
            <UButton
              :label="$t('common.done')"
              class="flex-1 justify-center"
              @click="resultOpen = false"
            />
          </div>
        </div>

        <div
          v-else
          class="flex flex-col items-center gap-3 text-center"
        >
          <div
            class="relative grid size-18 place-items-center rounded-full bg-error/10 text-error animate-pay-result-pop-error motion-reduce:animate-none"
          >
            <UIcon
              name="i-lucide-circle-alert"
              class="size-12"
            />
          </div>
          <p
            class="text-lg font-semibold animate-pay-rise-1 motion-reduce:animate-none"
          >
            {{ $t('checkout.result.failed') }}
          </p>
          <p
            class="text-sm text-muted animate-pay-rise-2 motion-reduce:animate-none"
          >
            {{ failure }}
          </p>
          <div
            class="mt-1 w-full animate-pay-rise-3 motion-reduce:animate-none"
          >
            <UButton
              :label="$t('common.close')"
              block
              @click="resultOpen = false"
            />
          </div>
        </div>
      </template>
    </UModal>

    <footer class="border-t border-default px-5 py-5 text-center text-xs text-muted print:fixed print:inset-x-0 print:bottom-0 print:bg-default print:py-[8mm]">
      {{ $t('checkout.poweredBy') }}
      <NuxtLink
        to="/"
        class="font-medium text-primary underline-offset-4 hover:underline"
      >
        {{ $t('app.brandWord.lead') }}{{ $t('app.brandWord.rest') }}
      </NuxtLink>
    </footer>
  </div>
</template>
