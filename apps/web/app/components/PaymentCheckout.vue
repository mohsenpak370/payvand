<script setup lang="ts">
const props = defineProps<{ source?: string }>()

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
  priceLamports,
  settled,
  paymentName,
  paymentDescription,
  paymentMemo,
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

const savedToProfile = ref(false)
const saving = ref(false)
const saveError = ref('')

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
  savedToProfile.value = false
  const current = payload.value
  if (!user.value || !current) return
  try {
    const saved = await api.listSavedPayments()
    savedToProfile.value = saved.some(item => item.payment_id === current.id)
  } catch {
    // Not worth an error of its own: the save action reports its own failures.
  }
}

async function saveToProfile() {
  const current = payload.value
  if (!current) return
  saving.value = true
  saveError.value = ''
  try {
    await api.savePayment(JSON.stringify(current))
    savedToProfile.value = true
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
  <div class="flex min-h-[100svh] flex-col print:min-h-[100vh] print:bg-default">
    <main class="flex flex-1 items-center justify-center px-5 py-10 print:p-[14mm]">
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

            <div class="mt-6 flex flex-col items-center gap-1">
              <UBadge
                v-if="discount"
                color="success"
                variant="subtle"
                size="sm"
              >
                {{ discount.percent_off }}% off
              </UBadge>
              <p class="flex items-baseline justify-center gap-2 text-4xl font-bold tabular-nums">
                {{ lamportsToSol(payableLamports) }}
                <span class="text-xl font-semibold text-muted">SOL</span>
              </p>
              <p
                v-if="discount"
                class="text-sm text-muted tabular-nums"
              >
                <span class="line-through">{{ lamportsToSol(priceLamports) }} SOL</span>
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
                  Paying {{ lamportsToSol(payableLamports) }} SOL from your allowance
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
                  :label="`Pay ${lamportsToSol(payableLamports)} SOL`"
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
                  :label="`Pay ${lamportsToSol(payableLamports)} SOL`"
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

            <div
              v-if="discount && !settled"
              class="mt-2 flex flex-wrap items-center justify-center gap-2 print:hidden"
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

            <!--
              The printed page is this code: on paper it is the only way to
              pay, and on a desktop it is how the phone takes over.
            -->
            <div
              v-if="!settled"
              class="mt-6 hidden w-full max-w-[240px] sm:block print:block"
            >
              <PaymentQr
                :payload="code"
                :size="240"
              />
              <p class="mt-3 text-center text-xs text-muted">
                {{ stage === 'pick' ? 'Or scan this with your phone' : 'Or scan it with a phone' }}
              </p>
            </div>

            <dl class="mt-6 w-full space-y-1 text-xs text-muted">
              <div class="flex justify-between gap-3">
                <dt>Seller</dt>
                <dd class="truncate font-mono">
                  {{ payload?.seller }}
                </dd>
              </div>
              <div class="flex justify-between gap-3">
                <dt>Network</dt>
                <dd class="truncate">
                  {{ networkLabel(payload?.cluster || '') }}
                </dd>
              </div>
              <div
                v-if="record"
                class="flex justify-between gap-3"
              >
                <dt>Status</dt>
                <dd class="truncate">
                  {{ statusLabel(record.status) }}
                </dd>
              </div>
            </dl>
          </div>
        </section>

        <div
          v-if="stage !== 'loading' && stage !== 'invalid'"
          class="mt-3 flex flex-wrap items-center justify-center gap-2 print:hidden"
        >
          <UButton
            v-if="savedToProfile"
            to="/profile"
            label="Saved to your profile"
            icon="i-lucide-bookmark-check"
            color="neutral"
            variant="ghost"
            size="sm"
          />
          <UButton
            v-else-if="user"
            label="Save to my profile"
            icon="i-lucide-bookmark-plus"
            color="neutral"
            variant="ghost"
            size="sm"
            :loading="saving"
            @click="saveToProfile"
          />
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

    <footer class="border-t border-default px-5 py-5 text-center text-xs text-muted">
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
