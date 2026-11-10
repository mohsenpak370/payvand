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
  autoSeconds,
  payableLamports,
  payableAmount,
  priceAmount,
  options,
  selected,
  selectCurrency,
  offersChoice,
  settled,
  paymentName,
  paymentDescription,
  paymentMemo,
  items,
  canPay,
  stage,
  load,
  pay,
  applyDiscount,
  clearDiscount,
  cancelAutoPay
} = usePaymentCheckout()

const heading = computed(() => paymentName.value || paymentMemo.value || 'Payment')

const discountOpen = ref(false)

// Written once and printed on the sheet: paper has nothing to click.
const detailRows = computed(() => [
  { label: 'Seller', value: String(payload.value?.seller ?? ''), mono: true },
  { label: 'Network', value: networkLabel(payload.value?.cluster || ''), mono: false },
  ...(record.value
    ? [{ label: 'Status', value: statusLabel(record.value.status), mono: false }]
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
  return opensApp(wallet) ? `Open in ${wallet.name}` : `Connect ${wallet.name}`
}

async function pick(wallet: NonNullable<typeof selectedWallet.value>) {
  selectWallet(wallet)
  await nextTick()
  if (canConnect.value && !connected.value) await connect()
}

/** Opening the code field is a decision to price this differently first. */
function openDiscount() {
  cancelAutoPay()
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
    saveError.value = apiErrorMessage(cause, 'Could not save this payment.')
  } finally {
    saving.value = false
  }
}

/**
 * The print dialog is the only Save-as-PDF the browsers agree on, and it keeps
 * the code as vectors — a rasterised PDF would scan badly once printed at
 * receipt size. The page is styled for paper, so this just opens it.
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
  title: () => (paymentName.value ? `${paymentName.value} — Payvand` : 'Payment — Payvand')
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
              Reading the payment…
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
              This payment link cannot be opened
            </h1>
            <p class="text-sm text-muted">
              {{ error }}
            </p>
            <NuxtLink
              to="/"
              class="mt-2 text-sm font-medium text-primary underline-offset-4 hover:underline"
            >
              Go to Payvand
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
              class="mt-4 w-full max-w-sm rounded-xl border border-default text-left"
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
                <span>Total</span>
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
                {{ discount.percent_off }}% off
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
                  title="Paid"
                  description="This invoice is settled — there is nothing left to do."
                />
                <p
                  v-if="signature || record?.settled_by"
                  class="overflow-hidden text-xs text-muted font-mono text-ellipsis"
                >
                  {{ signature ? `Signature ${signature}` : `Paid by ${record?.settled_by}` }}
                </p>
              </div>

              <!-- Paying silently from the allowance, with time to stop it. -->
              <div
                v-else-if="stage === 'auto'"
                class="rounded-xl border border-success/40 bg-success/5 p-4"
              >
                <p class="text-sm font-medium">
                  Paying {{ payableLabel }} from your allowance
                </p>
                <p class="mt-1 text-xs text-muted">
                  No wallet approval needed. Sending in {{ autoSeconds }}s.
                </p>
                <UButton
                  label="Cancel"
                  color="neutral"
                  variant="ghost"
                  size="xs"
                  class="mt-2"
                  @click="cancelAutoPay"
                />
              </div>

              <!-- The allowance already covers it, but the buyer cancelled once. -->
              <template v-else-if="stage === 'silent'">
                <UButton
                  :label="`Pay ${payableLabel}`"
                  icon="i-lucide-zap"
                  :loading="busy"
                  :disabled="!canPay"
                  block
                  @click="pay"
                />
                <p class="mt-2 text-xs text-muted">
                  Paid from your monthly allowance — no wallet popup.
                </p>
              </template>

              <!-- A connected wallet: one tap, straight to its approval. -->
              <template v-else-if="stage === 'wallet'">
                <UButton
                  :label="`Pay ${payableLabel}`"
                  icon="i-lucide-wallet"
                  :loading="busy"
                  :disabled="!canPay"
                  block
                  @click="pay"
                />
                <p class="mt-2 text-xs text-muted">
                  Approve in {{ selectedWallet?.name || 'your wallet' }}. Nothing is sent until you do.
                </p>
                <UAlert
                  v-if="budgetObstacle"
                  color="warning"
                  variant="subtle"
                  icon="i-lucide-circle-alert"
                  title="Your allowance cannot cover this"
                  :description="budgetObstacle"
                  class="mt-3 text-left"
                />
                <div
                  v-else-if="!budgetState"
                  class="mt-3 rounded-xl border border-dashed border-default p-3 text-left"
                >
                  <p class="text-xs font-medium">
                    Pay in one tap next time
                  </p>
                  <p class="mt-1 text-xs text-muted">
                    A monthly allowance lets payments inside it go through without any wallet popup.
                  </p>
                  <UButton
                    to="/budget"
                    label="Set a monthly allowance"
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
                class="flex flex-col gap-2 text-left"
              >
                <UAlert
                  color="info"
                  variant="subtle"
                  icon="i-lucide-key-round"
                  title="Finish setting up this device"
                  description="Your allowance needs one approval on this device before it can pay without a wallet popup. After that it is one tap."
                />
                <UButton
                  to="/budget"
                  label="Set up this device"
                  block
                />
                <UButton
                  :label="`Pay with ${selectedWallet?.name || 'your wallet'}`"
                  :loading="busy"
                  :disabled="!canPay"
                  color="neutral"
                  variant="soft"
                  block
                  @click="pay"
                />
              </div>

              <!-- Nothing connected: the wallets this browser can offer. -->
              <div
                v-else-if="stage === 'pick'"
                class="flex flex-col gap-2"
              >
                <p class="text-sm font-medium">
                  Choose a wallet to pay
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
                  No Solana wallet found in this browser. Install one and reload, or scan this
                  page's code with your phone.
                </p>
                <UButton
                  v-if="handheld && walletLink"
                  :to="walletLink"
                  label="Open in a Solana wallet"
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
                title="Payment failed"
                :description="failure"
                class="mt-3 text-left"
              />
            </div>

            <!-- Priced by a code: the invoice's own, or one typed here. -->
            <div
              v-if="payload?.seller_user && !discount && !settled"
              class="mt-3 w-full print:hidden"
            >
              <UButton
                v-if="!discountOpen"
                label="Have a discount code?"
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
                  placeholder="SAVE10"
                  :maxlength="32"
                  @focus="cancelAutoPay"
                />
                <UButton
                  label="Apply"
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
                class="mt-2 text-left"
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
                {{ discount.code }} · {{ discount.percent_off }}% off
              </UBadge>
              <UButton
                label="Remove"
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
                  <dt>{{ row.label }}</dt>
                  <dd
                    class="truncate"
                    :class="row.mono ? 'font-mono' : ''"
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
            :label="savedShop ? `Saved to ${savedShop.name}` : 'Saved to your shop'"
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
              aria-label="Shop to save this code in"
            />
            <UButton
              label="Save to my shop"
              icon="i-lucide-bookmark-plus"
              color="neutral"
              variant="ghost"
              size="sm"
              :loading="saving"
              @click="saveToShop"
            />
          </template>
          <UButton
            label="Save as PDF"
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
          title="Could not save the payment"
          :description="saveError"
          class="mt-2"
        />
      </div>
    </main>

    <footer class="border-t border-default px-5 py-5 text-center text-xs text-muted print:fixed print:inset-x-0 print:bottom-0 print:bg-default print:py-[8mm]">
      Powered by
      <NuxtLink
        to="/"
        class="font-medium text-primary underline-offset-4 hover:underline"
      >
        Payvand
      </NuxtLink>
    </footer>
  </div>
</template>
