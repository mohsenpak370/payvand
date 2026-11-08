<script setup lang="ts">
import type { FiatCurrency } from '~/composables/useFiat'

useSeoMeta({ title: 'Settings — Payvand' })
definePageMeta({ middleware: 'auth' })

// The money prices are shown in. Charges stay in SOL; this only changes the
// hint next to them, everywhere in the app.
const { currency, setCurrency } = useCurrencyPreference()
const rates = useSolRates()
const currencyChoice = computed({
  get: () => currency.value,
  set: (next: FiatCurrency) => setCurrency(next)
})
const rateHint = computed(() => {
  const rate = rates.value?.[currency.value]
  return rate ? formatFiat(rate, currency.value) : null
})

// Which parts of the app this person uses. Off leaves their shops where they
// are, and only takes the link out of the navigation.
const { shopsEnabled, setShopsEnabled } = useShopPreference()
const shopsChoice = computed({
  get: () => shopsEnabled.value,
  set: (next: boolean) => setShopsEnabled(next)
})
// Two radios rather than a switch: the answer is a choice, and each one can say
// what it means.
const SHOP_CHOICES = [
  { label: 'Yes — add a Shops link to the navigation', value: true },
  { label: 'No — I charge for one thing at a time', value: false }
]
</script>

<template>
  <UPageSection
    title="Settings"
    description="How prices are shown, and which parts of the app you want to see."
  >
    <div class="mx-auto flex w-full max-w-3xl min-w-0 flex-col gap-6">
      <!-- Currency of the hints shown next to every price. -->
      <UCard class="min-w-0">
        <template #header>
          <div>
            <h2 class="text-base font-semibold">
              Currency
            </h2>
            <p class="text-sm text-muted">
              Payments are charged in SOL. This is the money shown next to a price.
            </p>
          </div>
        </template>

        <div class="flex flex-col gap-2">
          <UFormField label="Show prices in">
            <USelect
              v-model="currencyChoice"
              :items="CURRENCY_OPTIONS"
              class="w-56"
            />
          </UFormField>
          <!-- Its own line: beside the select it hung halfway down the field. -->
          <p
            v-if="rateHint"
            class="text-xs text-muted tabular-nums"
          >
            1 SOL ≈ {{ rateHint }}
          </p>
        </div>
      </UCard>

      <!-- Shops: a catalogue of products instead of a typed amount. -->
      <UCard class="min-w-0">
        <template #header>
          <div>
            <h2 class="text-base font-semibold">
              Shops
            </h2>
            <p class="text-sm text-muted">
              A shop keeps the products you sell, so a payment is priced from a list rather than
              typed in every time.
            </p>
          </div>
        </template>

        <div class="flex flex-col gap-4">
          <div>
            <p class="text-sm font-medium">
              Use shops
            </p>
            <p class="text-sm text-muted">
              Turning them off only hides the link — any shop page you have already shared keeps
              working.
            </p>
          </div>

          <!-- The paragraph above names the group, so the legend would repeat
               it; `aria-label` gives the radiogroup the same name. -->
          <URadioGroup
            v-model="shopsChoice"
            :items="SHOP_CHOICES"
            aria-label="Use shops"
          />

          <div
            v-if="shopsEnabled"
            class="border-default flex justify-end border-t pt-4"
          >
            <UButton
              to="/shops"
              label="Open shops"
              icon="i-lucide-store"
              trailing-icon="i-lucide-arrow-right"
              color="neutral"
              variant="soft"
              size="sm"
            />
          </div>
        </div>
      </UCard>

      <p class="text-xs text-muted">
        Both of these follow this browser, not your account — a phone you sign in on keeps its own.
      </p>
    </div>
  </UPageSection>
</template>
