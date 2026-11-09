<script setup lang="ts">
useSeoMeta({ title: 'Scan and pay — Payvand' })

// The camera is this page's whole job: read a code, prove it matches its own
// details, then hand it to the shared checkout. Everything after that — which
// currency it charges, discounts, the monthly allowance, the wallet approval
// and the live status — is exactly what a shared link shows, so it lives in one
// component instead of a second copy here. That copy only ever understood SOL,
// which is why a USDC or EURC code scanned here used to fail outright.
//
// No account wall: signing in only unlocks saving the code and the silent
// allowance path, both of which the checkout handles for itself.

const route = useRoute()
const router = useRouter()

const scanned = ref('')
const scanError = ref('')
const manualPayload = ref('')
// Typing a code is the fallback, not the way in: the camera is on screen and
// the paste box only appears for someone whose phone will not focus.
const pasteOpen = ref(false)

async function load(raw: string) {
  scanError.value = ''
  const text = raw.trim()
  if (!text) return
  try {
    // Checked here as well as inside the checkout: a code that fails should
    // leave the buyer at the camera, not move them to a payment screen first.
    await verifyPaymentCode(parseQrPayload(text))
    scanned.value = text
    manualPayload.value = ''
  } catch (error) {
    scanError.value = error instanceof Error ? error.message : 'Could not read that code.'
  }
}

/** Back to the camera, and out of the deep link that opened this code. */
function scanAnother() {
  scanned.value = ''
  scanError.value = ''
  if (route.query.p) void router.replace({ query: { ...route.query, p: undefined } })
}

onMounted(async () => {
  // Arriving from a code read elsewhere: the link carries the invoice payload.
  const embedded = route.query.p
  if (typeof embedded === 'string' && embedded.length > 0) await load(embedded)
})
</script>

<template>
  <div>
    <!--
      One card at a time. The camera takes the whole page until a code is read,
      then it leaves: leaving it mounted next to the payment meant staring at
      your own camera while paying, and it kept the request for camera access
      alive for no reason. "Scan another" on the payment sheet brings it back.
    -->
    <UPageSection
      v-if="!scanned"
      title="Scan and pay"
      description="Point your camera at the seller's code, check the amount, and approve the payment in your wallet."
    >
      <div class="mx-auto w-full max-w-2xl">
        <UCard>
          <template #header>
            <h2 class="text-xl font-semibold">
              Scan the code
            </h2>
          </template>

          <div class="flex flex-col gap-4">
            <ClientOnly>
              <QrScanner @decoded="load" />
              <template #fallback>
                <!-- Same dark panel the scanner renders, so the card does not
                     flash a pale block before the camera code arrives. -->
                <div class="aspect-[4/5] w-full animate-pulse rounded-2xl bg-neutral-900/80 sm:aspect-square" />
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

            <UButton
              v-if="!pasteOpen"
              label="Can't scan it? Paste the code"
              icon="i-lucide-chevron-down"
              color="neutral"
              variant="ghost"
              size="sm"
              class="self-start"
              :aria-expanded="false"
              @click="pasteOpen = true"
            />

            <UFormField
              v-else
              label="Paste the code text"
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
      </div>
    </UPageSection>

    <!--
      The same sheet a shared link opens: one price in the currency the seller
      chose, and the one action this browser can take to pay it. Nothing to
      keep and nothing to print, though — this code is being paid now.
    -->
    <PaymentCheckout
      v-else
      :source="scanned"
      :permanent="false"
    >
      <template #actions>
        <UButton
          label="Scan another"
          icon="i-lucide-scan-line"
          color="neutral"
          variant="ghost"
          size="sm"
          @click="scanAnother"
        />
      </template>
    </PaymentCheckout>
  </div>
</template>
