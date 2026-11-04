<script setup lang="ts">
import { address } from '@vue-solana/nuxt/kit'
import type { PaymentQrPayload } from '~/composables/usePaymentApi'

// The shared link is the product, not a checkout: one sheet with what is being
// sold, its price and the code to pay it with. Nothing to fill in and nothing
// to press — the customer scans the code with their wallet. It also has to
// survive being printed, so the app header and footer stay out of the way.
definePageMeta({ header: false, footer: false })

const api = usePaymentApi()
const { user, ensureSession } = useAuth()
const route = useRoute()

const payload = ref<PaymentQrPayload | null>(null)
// The seller's own code, verbatim: re-encoding it from the parsed fields would
// work, but showing exactly what they handed out is the point of the page.
const code = ref('')
const loading = ref(true)
const scanError = ref('')

// Name and description travel inside the code itself, next to the amount.
const paymentName = computed(() => payload.value?.name || '')
const paymentDescription = computed(() => payload.value?.description || '')
const paymentMemo = computed(() => payload.value?.memo || '')
const heading = computed(() => paymentName.value || paymentMemo.value || 'Payment')

// A discount the seller attached while creating the invoice rides in the code.
// The card shows the tag and the two prices but never the code itself: the
// buyer does not need it, and the scan screen applies it on its own.
const priceLamports = computed(() => payload.value?.amount_lamports ?? 0)
const discount = computed(() => payload.value?.discount ?? null)
const payableLamports = computed(() =>
  discount.value
    ? discountedLamports(priceLamports.value, discount.value.percent_off)
    : priceLamports.value
)

// Saving belongs to the viewer, not to the invoice: the page stays open to
// whoever the link reaches.
const savedToProfile = ref(false)
const saving = ref(false)
const saveError = ref('')

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
  } catch (error) {
    saveError.value = apiErrorMessage(error, 'Could not save this payment.')
  } finally {
    saving.value = false
  }
}

async function load(raw: string) {
  loading.value = true
  scanError.value = ''
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
      throw new Error('These details do not match the code. Ask the seller for a new link.')
    }

    // The sheet shows only what the code says, so the page needs no chain
    // read: the derived address above is what proves the details are genuine.
    code.value = raw.trim()
    payload.value = parsed
  } catch (error) {
    scanError.value = error instanceof Error ? error.message : 'Could not read this payment link.'
  } finally {
    loading.value = false
  }
}

/**
 * The print dialog is the only Save-as-PDF the browsers agree on, and it keeps
 * the code as vectors — a rasterised PDF would scan badly once printed at
 * receipt size. The page is styled for paper below, so this just opens it.
 */
function saveAsPdf() {
  window.print()
}

onMounted(async () => {
  const embedded = route.query.p
  if (typeof embedded === 'string' && embedded.length > 0) {
    await load(embedded)
  } else {
    loading.value = false
  }

  // Anyone can view the payment; only a signed-in visitor can keep a copy.
  await ensureSession()
  await refreshSaved()
})

// The suggested PDF filename is the page title, so the name leads it.
useSeoMeta({
  title: () => (paymentName.value ? `${paymentName.value} — Payvand` : 'Payment — Payvand')
})

// Printing from the dark theme would put an ink-hungry slab on paper: the
// page prints on white regardless of the theme in use.
useHead({ bodyAttrs: { class: 'print:bg-white' } })
</script>

<template>
  <div class="flex min-h-[100svh] flex-col print:min-h-0 print:bg-white print:text-black">
    <main class="flex flex-1 items-center justify-center px-5 py-10 print:py-0">
      <div class="w-full max-w-sm">
        <!--
          One sheet, sized like the receipt it may become: name, price, code.
          Everything interactive lives outside it so printing keeps a clean
          page.
        -->
        <section class="rounded-2xl border border-default bg-default p-6 shadow-sm print:break-inside-avoid print:border-neutral-300 print:bg-white print:text-black print:shadow-none">
          <div
            v-if="loading"
            class="flex flex-col items-center gap-4 py-8"
          >
            <div class="aspect-square w-full max-w-[280px] animate-pulse rounded-xl bg-elevated/50" />
            <p class="text-sm text-muted">
              Reading the payment…
            </p>
          </div>

          <div
            v-else-if="scanError"
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
              {{ scanError }}
            </p>
            <NuxtLink
              to="/"
              class="mt-2 text-sm font-medium text-primary underline-offset-4 hover:underline"
            >
              Go to Payvand
            </NuxtLink>
          </div>

          <div
            v-else-if="!payload"
            class="flex flex-col items-center gap-2 py-6 text-center"
          >
            <UIcon
              name="i-lucide-qr-code"
              class="size-8 text-muted"
            />
            <h1 class="text-base font-semibold">
              No payment in this link
            </h1>
            <p class="text-sm text-muted">
              Payment pages open from the link the seller shares. Ask them for it,
              or scan their QR code.
            </p>
            <NuxtLink
              to="/scan"
              class="mt-2 text-sm font-medium text-primary underline-offset-4 hover:underline"
            >
              Scan and pay instead
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

            <div class="mt-6 w-full max-w-[280px]">
              <PaymentQr
                :payload="code"
                :size="280"
              />
            </div>

            <p class="mt-4 text-xs text-muted print:text-neutral-600">
              Scan to pay
            </p>
          </div>
        </section>

        <div
          v-if="payload && !scanError"
          class="mt-2 flex flex-wrap items-center justify-center gap-2 print:hidden"
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

    <footer class="border-t border-default px-5 py-5 text-center text-xs text-muted print:border-neutral-300 print:text-neutral-600">
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
