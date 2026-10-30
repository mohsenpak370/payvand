<script setup lang="ts">
import { address } from '@vue-solana/nuxt/kit'
import type { DiscountCode, PaymentRecord } from '~/composables/usePaymentApi'

useSeoMeta({ title: 'Sell — Payvand' })

definePageMeta({ middleware: 'auth' })

const { cluster } = useRuntimeConfig().public
const api = usePaymentApi()
const { publicKey, connected } = useSolanaWallet()

const amountSol = ref('0.01')
const memo = ref('Coffee')
const busy = ref(false)
const failure = ref('')
const record = ref<PaymentRecord | null>(null)
const qrPayload = ref('')

// Discount codes, and the price of the open invoice — after settlement the
// chain reports the *received* amount, so the full price is kept here.
const discounts = ref<DiscountCode[]>([])
const newCode = ref('')
const newPercent = ref('10')
const codeBusy = ref(false)
const codeFailure = ref('')
const priceLamports = ref(0)
const paidCode = ref<string | null>(null)

const canAddCode = computed(() =>
  !codeBusy.value
  && newCode.value.trim().length >= 3
  && Number(newPercent.value) >= 1
  && Number(newPercent.value) <= 99
)

const lamports = computed(() => solToLamports(Number(amountSol.value)))
// Creating a code needs the seller's address, never a signature: the invoice
// address is derived locally and the account only comes into existence (and
// is only funded) when the customer pays.
const canCreate = computed(() => connected.value && !busy.value && lamports.value > 0)

const statusColor = (status: string) =>
  status === 'settled' ? 'success' : status === 'missing' || status === 'pending' ? 'warning' : 'neutral'

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

  busy.value = true
  try {
    // Nothing to sign: derive the invoice address from the amount locally,
    // then record the details. The customer's payment funds the account.
    const reference = createReference()
    const paymentPda = await derivePaymentPda(seller, reference, lamports.value)

    const created = await api.createPayment({
      payment_pda: paymentPda,
      seller,
      reference,
      amount_lamports: lamports.value,
      memo: memo.value,
      cluster
    })

    record.value = created.payment
    qrPayload.value = created.qrPayload
    priceLamports.value = lamports.value
    paidCode.value = null
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
async function watchTargets(): Promise<{ pda: string, code: string | null }[]> {
  const current = record.value
  if (!current) return []

  const targets = [{ pda: current.payment_pda, code: null as string | null }]
  const seller = address(current.seller)
  for (const discount of discounts.value) {
    const amount = discountedLamports(priceLamports.value, discount.percent_off)
    if (amount <= 0 || amount === priceLamports.value) continue
    const pda = await derivePaymentPda(seller, address(current.reference), amount)
    targets.push({ pda, code: discount.code })
  }
  return targets
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
      record.value = settled
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
}

let timer: ReturnType<typeof setInterval> | undefined

onMounted(() => {
  loadDiscounts()
  timer = setInterval(refreshStatus, 5_000)
})

onBeforeUnmount(() => {
  if (timer) clearInterval(timer)
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
          <UFormField label="Amount (SOL)">
            <UInput
              v-model="amountSol"
              type="number"
              min="0"
              step="0.001"
              class="w-full"
              placeholder="0.01"
            />
          </UFormField>

          <UFormField
            label="Memo"
            help="A short note your customer will see — like “Coffee for two”."
          >
            <UTextarea
              v-model="memo"
              :rows="2"
              class="w-full"
              placeholder="What is this payment for?"
            />
          </UFormField>

          <div class="flex items-center justify-between gap-3 text-sm text-muted">
            <span>{{ networkLabel(cluster) }}</span>
            <span>{{ lamportsToSol(lamports) }} SOL</span>
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

          <div class="flex items-center gap-2">
            <UBadge
              :color="statusColor(record.status)"
              variant="subtle"
              size="lg"
            >
              {{ statusLabel(record.status) }}
            </UBadge>
            <span class="text-sm text-muted">
              {{ lamportsToSol(record.amount_lamports) }} SOL
              <template v-if="record.memo"> · {{ record.memo }}</template>
            </span>
            <UBadge
              v-if="paidCode"
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
  </UPageSection>
</template>
