<script setup lang="ts">
import type { FiatCurrency } from '~/composables/useFiat'

const { t, locale, locales, setLocale } = useI18n()

// Reactive: this is the page where the language changes, so the tab title has
// to follow the switch without a navigation.
useSeoMeta({ title: computed(() => t('settings.metaTitle')) })
definePageMeta({ middleware: 'auth' })

// The language of the interface itself. `setLocale` writes the cookie that
// keeps the choice after a reload, and the direction on <html> follows the
// locale, so Persian lays the whole app out right to left. Payment links carry
// no locale, so switching never breaks one that has been shared or printed.
type LocaleCode = Parameters<typeof setLocale>[0]
const languageChoice = computed<LocaleCode>({
  get: () => locale.value as LocaleCode,
  set: next => setLocale(next)
})
const languageItems = computed(() => locales.value.map(entry => ({
  label: entry.name ?? entry.code,
  value: entry.code
})))

// The money prices are shown in. Charges stay in SOL; this only changes the
// hint next to them, everywhere in the app.
const { currency, setCurrency } = useCurrencyPreference()
const rates = useSolRates()
const currencyChoice = computed({
  get: () => currency.value,
  set: (next: FiatCurrency) => setCurrency(next)
})
// The labels are copy, so they come from the locale; the values are the API's
// own currency codes.
const currencyItems = computed(() => FIAT_CURRENCIES.map(code => ({
  label: t(`fiat.${code}`),
  value: code
})))
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
const shopChoices = computed(() => [
  { label: t('settings.shopChoices.on'), value: true },
  { label: t('settings.shopChoices.off'), value: false }
])

// A payment settling — or failing — makes a short sound, because the person
// paying is holding the phone and looking at the shop, not at the screen.
const { enabled: soundsEnabled, setEnabled: setSoundsEnabled } = useTransactionSounds()
const soundsChoice = computed({
  get: () => soundsEnabled.value,
  set: (next: boolean) => setSoundsEnabled(next)
})
</script>

<template>
  <UPageSection
    :title="$t('settings.title')"
    :description="$t('settings.description')"
  >
    <div class="mx-auto flex w-full max-w-3xl min-w-0 flex-col gap-6">
      <!-- Language of the app itself, and the direction that follows it. -->
      <UCard class="min-w-0">
        <template #header>
          <div>
            <h2 class="text-base font-semibold">
              {{ $t('settings.languageTitle') }}
            </h2>
            <p class="text-sm text-muted">
              {{ $t('settings.languageDescription') }}
            </p>
          </div>
        </template>

        <UFormField :label="$t('settings.languageLabel')">
          <USelect
            v-model="languageChoice"
            :items="languageItems"
            class="w-56"
          />
        </UFormField>
      </UCard>

      <!-- Currency of the hints shown next to every price. -->
      <UCard class="min-w-0">
        <template #header>
          <div>
            <h2 class="text-base font-semibold">
              {{ $t('settings.currencyTitle') }}
            </h2>
            <p class="text-sm text-muted">
              {{ $t('settings.currencyDescription') }}
            </p>
          </div>
        </template>

        <div class="flex flex-col gap-2">
          <UFormField :label="$t('settings.currencyLabel')">
            <USelect
              v-model="currencyChoice"
              :items="currencyItems"
              class="w-56"
            />
          </UFormField>
          <!-- Its own line: beside the select it hung halfway down the field. -->
          <p
            v-if="rateHint"
            class="text-xs text-muted tabular-nums"
          >
            {{ $t('settings.rateHint', { rate: rateHint }) }}
          </p>
        </div>
      </UCard>

      <!-- Shops: a catalogue of products instead of a typed amount. -->
      <UCard class="min-w-0">
        <template #header>
          <div>
            <h2 class="text-base font-semibold">
              {{ $t('settings.shopsTitle') }}
            </h2>
            <p class="text-sm text-muted">
              {{ $t('settings.shopsDescription') }}
            </p>
          </div>
        </template>

        <div class="flex flex-col gap-4">
          <div>
            <p class="text-sm font-medium">
              {{ $t('settings.useShops') }}
            </p>
            <p class="text-sm text-muted">
              {{ $t('settings.shopsOffHint') }}
            </p>
          </div>

          <!-- The paragraph above names the group, so the legend would repeat
               it; `aria-label` gives the radiogroup the same name. -->
          <URadioGroup
            v-model="shopsChoice"
            :items="shopChoices"
            :aria-label="$t('settings.useShops')"
          />

          <div
            v-if="shopsEnabled"
            class="border-default flex justify-end border-t pt-4"
          >
            <UButton
              to="/shops"
              :label="$t('settings.openShops')"
              icon="i-lucide-store"
              trailing-icon="i-lucide-arrow-right"
              color="neutral"
              variant="soft"
              size="sm"
            />
          </div>
        </div>
      </UCard>

      <!-- Sounds: the two tones a payment makes. -->
      <UCard class="min-w-0">
        <template #header>
          <div>
            <h2 class="text-base font-semibold">
              {{ $t('settings.soundsTitle') }}
            </h2>
            <p class="text-sm text-muted">
              {{ $t('settings.soundsDescription') }}
            </p>
          </div>
        </template>

        <div class="flex flex-col gap-2">
          <UFormField :label="$t('settings.soundsLabel')">
            <USwitch
              v-model="soundsChoice"
              :label="$t('settings.soundsSwitch')"
            />
          </UFormField>
        </div>
      </UCard>

      <p class="text-xs text-muted">
        {{ $t('settings.browserNote') }}
      </p>
    </div>
  </UPageSection>
</template>
