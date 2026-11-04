<script setup lang="ts">
import { address } from '@vue-solana/nuxt/kit'
import type { DiscountPreview, PaymentQrPayload, PaymentRecord } from '~/composables/usePaymentApi'
import type { BudgetState } from '~/composables/useBudget'

useSeoMeta({ title: 'Pay — Payvand' })

// No account wall: anyone holding the payment link can open this page,
// connect a wallet and pay. Sign-in only unlocks the silent budget path.

const api = usePaymentApi()
const { user, ensureSession } = useAuth()
const route = useRoute()
const { client } = useSolanaClient()
const { publicKey, connected, canSignTransaction } = useSolanaWallet()
const { execute, status: txStatus, error: txError, loading: txLoading } = useSolanaSignAndSendTransaction()

const payload = ref<PaymentQrPayload | null>(null)
const record = ref<PaymentRecord | null>(null)
const manualPayload = ref('')
const scanError = ref('')
const failure = ref('')
const busy = ref(false)
const paidSignature = ref<string | null>(null)

// Discount code: the applied price plus the address that price pays into —
// the program derives a different address per amount, so this is what gets
// watched and paid once a code is accepted.
const discount = ref<DiscountPreview | null>(null)
const discountInput = ref('')
const discountBusy = ref(false)
const discountError = ref('')
const activePda = ref<string | null>(null)

const payableLamports = computed(() =>
  discount.value?.discounted_lamports ?? payload.value?.amount_lamports ?? 0
)

// Name and description ride in the code itself: the chain stores only the
// memo, so a record read back by id comes without them.
const paymentName = computed(() => record.value?.name || payload.value?.name || '')
const paymentDescription = computed(
  () => record.value?.description || payload.value?.description || ''
)
const paymentMemo = computed(() => record.value?.memo || payload.value?.memo || '')

// Saving keeps a copy of this code on the signed-in account. The page itself
// stays public, so the link works for whoever it is shared with.
const savedToProfile = ref(false)
const saving = ref(false)
const saveError = ref('')

// Signing in must come back to this exact payment, code included.
const signinLink = computed(() => ({ path: '/signin', query: { next: route.fullPath } }))

async function refreshSaved() {
  savedToProfile.value = false
  const current = payload.value
  if (!user.value || !current) return
  try {
    const saved = await api.listSavedPayments()
    savedToProfile.value = saved.some(item => item.payment_id === current.id)
  } catch {
    // Not worth an error of its own: the save button reports its own failures.
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
  } catch (error) {
    saveError.value = apiErrorMessage(error, 'Could not save this payment.')
  } finally {
    saving.value = false
  }
}

// Monthly budget: when it can cover the price, the payment is signed by the
// local session key instead of the wallet — no popup at all.
const budgetApi = useBudget()
const budgetState = ref<BudgetState | null>(null)
const budgetObstacle = ref<string | null>(null)

const canPay = computed(
  () =>
    connected.value
    && canSignTransaction.value
    && !busy.value
    && !txLoading.value
    // "missing" is the normal state of a fresh code: the invoice account is
    // created inside the buyer's own transaction.
    && (record.value?.status === 'pending' || record.value?.status === 'missing')
)

const statusColor = (status: string) =>
  status === 'settled' ? 'success' : status === 'missing' || status === 'pending' ? 'warning' : 'neutral'

/** Can the connected wallet's budget cover this payment without a popup? */
async function refreshBudget() {
  budgetState.value = null
  budgetObstacle.value = null
  const payer = publicKey.value
  if (!payer) return
  try {
    const state = await budgetApi.loadBudget(payer)
    if (!state) return
    budgetState.value = state
    const amount = payableLamports.value
    budgetObstacle.value = amount > 0
      ? await budgetApi.silentPaymentObstacle(state, amount)
      : null
  } catch {
    // A flaky RPC read must never block paying — the wallet path still works.
  }
}

watch(publicKey, refreshBudget)

async function load(raw: string) {
  scanError.value = ''
  failure.value = ''
  paidSignature.value = null
  record.value = null
  payload.value = null
  activePda.value = null
  discount.value = null
  discountInput.value = ''
  discountError.value = ''

  try {
    const parsed = parseQrPayload(raw.trim())

    // The program binds the price into the address, so deriving it here and
    // comparing proves the details were not tampered with.
    const derived = await derivePaymentPda(
      address(parsed.seller),
      address(parsed.reference),
      parsed.amount_lamports
    )
    if (derived !== parsed.id) {
      throw new Error('This payment code does not match its details. Ask the seller for a new one.')
    }

    const status = await api.getPayment(parsed.id, {
      cluster: parsed.cluster,
      endpoint: parsed.endpoint
    })
    payload.value = parsed
    record.value = status
    activePda.value = parsed.id
    await refreshBudget()
    await refreshSaved()
  } catch (error) {
    scanError.value = error instanceof Error ? error.message : 'Could not read that code.'
  }
}

async function pay() {
  const current = payload.value
  const payer = publicKey.value
  if (!current || !payer) return

  failure.value = ''
  busy.value = true
  try {
    const seller = address(current.seller)
    const reference = address(current.reference)
    // Full price unless a code was applied. `load()` already proved the code
    // matches its details; here the amount simply derives the address that
    // gets paid, so what is paid is exactly what was shown.
    const amount = payableLamports.value
    const payment = await derivePaymentPda(seller, reference, amount)

    // The budget path skips the wallet entirely: the local session key signs
    // and the program enforces the monthly limit. Anything else falls back
    // to a normal wallet approval.
    let state: BudgetState | null = null
    try {
      state = await budgetApi.loadBudget(payer)
    } catch {
      state = null
    }
    const silent
      = state !== null
        && (await budgetApi.silentPaymentObstacle(state, amount)) === null

    let signature: string
    if (state && silent) {
      signature = await budgetApi.sendSilentPayment(payer, {
        payment,
        seller,
        reference,
        amount,
        memo: current.memo || ''
      })
    } else {
      const instruction = buildInstruction(
        'settle_payment',
        { amount, memo: current.memo || '' },
        { payer, payment, seller, reference }
      )
      const wire = await compileWireTransaction({ instruction, payer, rpc: client.rpc })
      signature = await execute(wire, {
        confirm: true,
        confirmation: { commitment: 'confirmed', timeoutMs: 120_000 }
      })
    }
    paidSignature.value = signature

    record.value = await api.getPayment(payment, {
      cluster: current.cluster,
      endpoint: current.endpoint
    })
  } catch (error) {
    failure.value = error instanceof Error ? error.message : 'The payment could not be sent.'
  } finally {
    busy.value = false
  }
}

/** Ask the API to price the invoice with the buyer's code. */
async function applyDiscount() {
  const current = payload.value
  if (!current?.seller_user) return

  discountBusy.value = true
  discountError.value = ''
  try {
    const preview = await api.validateDiscount({
      code: discountInput.value,
      seller_user: current.seller_user,
      price_lamports: current.amount_lamports
    })
    discount.value = preview
    discountInput.value = ''
    // The discounted amount derives its own address — watch and pay that.
    activePda.value = await derivePaymentPda(
      address(current.seller),
      address(current.reference),
      preview.discounted_lamports
    )
    await refreshBudget()
  } catch (error) {
    discountError.value = apiErrorMessage(error, 'That discount code could not be applied.')
  } finally {
    discountBusy.value = false
  }
}

function clearDiscount() {
  discount.value = null
  discountInput.value = ''
  discountError.value = ''
  activePda.value = payload.value?.id ?? null
  refreshBudget()
}

function reset() {
  payload.value = null
  record.value = null
  manualPayload.value = ''
  scanError.value = ''
  failure.value = ''
  paidSignature.value = null
  activePda.value = null
  discount.value = null
  discountInput.value = ''
  discountError.value = ''
  savedToProfile.value = false
  saveError.value = ''
}

let timer: ReturnType<typeof setInterval> | undefined

onMounted(async () => {
  // Arriving from a scanned QR: the link carries the invoice payload.
  const embedded = route.query.p
  if (typeof embedded === 'string' && embedded.length > 0) {
    await load(embedded)
  }

  // Anyone can pay, but only a signed-in visitor can save the code.
  await ensureSession()
  await refreshSaved()

  timer = setInterval(async () => {
    const pda = activePda.value
    if (!payload.value || !pda || paidSignature.value) return
    try {
      record.value = await api.getPayment(pda, {
        cluster: payload.value.cluster,
        endpoint: payload.value.endpoint
      })
    } catch {
      // Keep the last known status; the next tick retries.
    }
  }, 5_000)
})

onBeforeUnmount(() => {
  if (timer) clearInterval(timer)
})
</script>

<template>
  <UPageSection
    title="Scan and pay"
    description="Point your camera at the seller's code, check the amount, and approve the payment in your wallet."
  >
    <div class="grid gap-6 lg:grid-cols-2">
      <UCard>
        <template #header>
          <div class="flex items-center justify-between gap-3">
            <h2 class="text-base font-semibold">
              1. Scan the code
            </h2>
            <UButton
              v-if="payload"
              label="Scan another"
              color="neutral"
              variant="soft"
              size="sm"
              @click="reset"
            />
          </div>
        </template>

        <div class="flex flex-col gap-4">
          <ClientOnly>
            <QrScanner
              v-if="!payload"
              @decoded="load"
            />
            <template #fallback>
              <div class="aspect-square w-full animate-pulse rounded-xl bg-elevated/50" />
            </template>
          </ClientOnly>

          <UAlert
            v-if="scanError"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            title="Could not read the code"
            :description="scanError"
          />

          <UFormField
            v-if="!payload"
            label="Or paste the code text"
          >
            <div class="flex w-full flex-col gap-2">
              <UTextarea
                v-model="manualPayload"
                :rows="3"
                class="w-full font-mono text-xs"
                placeholder="{&quot;v&quot;:1,&quot;id&quot;:&quot;…&quot;}"
              />
              <UButton
                label="Load"
                color="neutral"
                variant="soft"
                class="self-start"
                @click="load(manualPayload)"
              />
            </div>
          </UFormField>
        </div>
      </UCard>

      <UCard>
        <template #header>
          <div class="flex items-center justify-between gap-3">
            <h2 class="text-base font-semibold">
              2. Pay
            </h2>
            <WalletConnect v-if="payload" />
          </div>
        </template>

        <div
          v-if="payload && record"
          class="flex flex-col gap-4"
        >
          <div class="rounded-xl border border-default p-4">
            <p
              v-if="paymentName"
              class="text-sm font-medium"
            >
              {{ paymentName }}
            </p>
            <p class="text-2xl font-semibold">
              {{ lamportsToSol(record.status === 'settled' ? record.amount_lamports : payableLamports) }} SOL
              <span
                v-if="discount"
                class="ml-2 text-base font-normal text-muted line-through"
              >
                {{ lamportsToSol(payload.amount_lamports) }} SOL
              </span>
            </p>
            <p
              v-if="paymentDescription"
              class="mt-1 text-sm text-muted"
            >
              {{ paymentDescription }}
            </p>
            <p
              v-else
              class="mt-1 text-sm text-muted"
            >
              {{ paymentMemo || 'No memo' }}
            </p>
            <div
              v-if="discount"
              class="mt-2 flex flex-wrap items-center gap-2"
            >
              <UBadge
                color="success"
                variant="subtle"
              >
                {{ discount.code }} · {{ discount.percent_off }}% off
              </UBadge>
              <span class="text-xs text-muted">
                You save {{ lamportsToSol(discount.saved_lamports) }} SOL
              </span>
              <UButton
                label="Remove"
                color="neutral"
                variant="ghost"
                size="xs"
                @click="clearDiscount"
              />
            </div>
          </div>

          <div
            v-if="payload.seller_user && !discount && record.status !== 'settled'"
            class="flex items-end gap-3"
          >
            <UFormField
              label="Discount code"
              class="flex-1"
              help="Optional — enter the code the seller gave you."
            >
              <UInput
                v-model="discountInput"
                class="w-full"
                placeholder="SAVE10"
                :maxlength="32"
              />
            </UFormField>
            <UButton
              label="Apply"
              :loading="discountBusy"
              :disabled="discountInput.trim().length < 3"
              @click="applyDiscount"
            />
          </div>

          <UAlert
            v-if="discountError"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            title="Discount not applied"
            :description="discountError"
          />

          <dl class="space-y-1 text-xs text-muted">
            <div class="flex justify-between gap-3">
              <dt>Seller</dt>
              <dd class="truncate font-mono">
                {{ payload.seller }}
              </dd>
            </div>
            <div class="flex justify-between gap-3">
              <dt>Network</dt>
              <dd class="truncate">
                {{ networkLabel(payload.cluster) }}
              </dd>
            </div>
          </dl>

          <div class="flex items-center gap-2">
            <UBadge
              :color="statusColor(record.status)"
              variant="subtle"
              size="lg"
            >
              {{ statusLabel(record.status) }}
            </UBadge>
            <span
              v-if="record.settled_by"
              class="truncate text-xs text-muted font-mono"
            >
              paid by {{ record.settled_by }}
            </span>
          </div>

          <!--
            Saving is optional and belongs to the viewer, not the invoice: the
            same page stays open to anyone the link is shared with.
          -->
          <UButton
            v-if="savedToProfile"
            to="/profile"
            label="Saved — open it in your profile"
            icon="i-lucide-bookmark-check"
            color="success"
            variant="soft"
            size="sm"
            class="self-start"
          />
          <UButton
            v-else-if="user"
            label="Save this payment to my profile"
            icon="i-lucide-bookmark-plus"
            color="neutral"
            variant="soft"
            size="sm"
            class="self-start"
            :loading="saving"
            @click="saveToProfile"
          />
          <UButton
            v-else
            :to="signinLink"
            label="Sign in to save this payment"
            icon="i-lucide-log-in"
            color="neutral"
            variant="soft"
            size="sm"
            class="self-start"
          />

          <UAlert
            v-if="saveError"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            title="Could not save the payment"
            :description="saveError"
          />

          <UAlert
            v-if="budgetState && !budgetObstacle"
            color="success"
            variant="subtle"
            icon="i-lucide-zap"
            title="Pays from your monthly budget"
            description="No wallet popup needed — this payment comes straight out of your budget."
          />
          <UAlert
            v-else-if="budgetState && budgetObstacle"
            color="warning"
            variant="subtle"
            icon="i-lucide-circle-alert"
            title="Budget not usable for this payment"
            :description="`${budgetObstacle} You will approve this one in your wallet.`"
          />
          <div
            v-else
            class="rounded-xl border border-dashed border-default p-4 text-sm"
          >
            <p class="font-medium">
              Pay without approvals
            </p>
            <p class="mt-1 text-muted">
              Set a monthly limit once — payments inside it need no wallet popups.
            </p>
            <UButton
              to="/budget"
              label="Set up a monthly budget"
              color="primary"
              variant="soft"
              size="xs"
              class="mt-2"
            />
          </div>

          <UAlert
            v-if="failure"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            :title="txError?.code ?? 'Payment failed'"
            :description="failure"
          />

          <UButton
            v-if="record.status !== 'settled'"
            label="Pay now"
            icon="i-lucide-wallet"
            :loading="busy"
            :disabled="!canPay"
            block
            @click="pay"
          />

          <UAlert
            v-if="paidSignature"
            color="success"
            variant="subtle"
            icon="i-lucide-circle-check"
            title="Payment sent"
            :description="paidSignature"
          />

          <p
            v-if="txStatus !== 'idle'"
            class="text-xs text-muted"
          >
            Wallet status: {{ txStatus }}
          </p>
        </div>

        <div
          v-else
          class="py-10 text-center text-sm text-muted"
        >
          Scan a payment code to see the payment here.
        </div>
      </UCard>
    </div>
  </UPageSection>
</template>
