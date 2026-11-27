<script setup lang="ts">
import type { ButtonProps } from '@nuxt/ui'

const { t } = useI18n()
const { cluster } = useRuntimeConfig().public

// "Running on devnet" means nothing to a shop owner; say what it costs them.
const ctaTitle = computed(() =>
  cluster === 'devnet' || cluster === 'testnet'
    ? t('common.testMode')
    : t('common.liveMode')
)

const heroLinks = computed<ButtonProps[]>(() => [{
  label: t('home.hero.createCta'),
  to: '/payment',
  icon: 'i-lucide-qr-code',
  size: 'xl'
}, {
  label: t('home.hero.scanCta'),
  to: '/scan',
  icon: 'i-lucide-scan-line',
  size: 'xl',
  color: 'neutral',
  variant: 'subtle'
}])

// What the hero promises, in the three things a shop owner cares about
// without knowing any of the words: it is easy, it costs little, and the
// money is there while the customer is still at the counter.
const proofs = computed(() => [
  { key: 'easy', icon: 'i-lucide-smile', label: t('home.hero.proof.easy') },
  { key: 'lowFee', icon: 'i-lucide-piggy-bank', label: t('home.hero.proof.lowFee') },
  { key: 'instant', icon: 'i-lucide-zap', label: t('home.hero.proof.instant') }
])

const steps = computed(() => [{
  icon: 'i-lucide-monitor-smartphone',
  title: t('home.how.create.title'),
  description: t('home.how.create.description')
}, {
  icon: 'i-lucide-scan-qr-code',
  title: t('home.how.scan.title'),
  description: t('home.how.scan.description')
}, {
  icon: 'i-lucide-circle-check',
  title: t('home.how.paid.title'),
  description: t('home.how.paid.description')
}])

const FAQ_COUNT = 10

const FAQ_LINKS: Record<number, string[]> = {
  1: ['payvandVsWallet', 'stablecoins', 'checkingBeforeSign'],
  2: ['cryptoWallet'],
  3: ['payingWithWallet'],
  5: ['paymentCosts', 'fee'],
  6: ['fee'],
  9: ['monthlyAllowance'],
  10: ['testMoney', 'switchToDevnet']
}

/** Where each link in `home.faq.links` points on the learning pages. */
const LEARN_GUIDES: Record<string, string> = {
  payvandVsWallet: '/learn/what-payvand-adds-to-a-wallet',
  payingWithWallet: '/learn/paying-with-a-wallet',
  stablecoins: '/learn/stablecoins-explained',
  cryptoWallet: '/learn/what-is-a-crypto-wallet',
  paymentCosts: '/learn/what-a-payment-costs',
  fee: '/learn/what-is-a-fee',
  monthlyAllowance: '/learn/monthly-allowance-explained',
  testMoney: '/learn/test-money-on-devnet',
  switchToDevnet: '/learn/switch-to-devnet',
  checkingBeforeSign: '/learn/checking-before-you-sign'
}

const faqs = computed(() => Array.from({ length: FAQ_COUNT }, (_, index) => ({
  label: t(`home.faq.q${index + 1}`),
  content: t(`home.faq.a${index + 1}`),
  links: (FAQ_LINKS[index + 1] ?? []).map(key => ({
    label: t(`home.faq.links.${key}`),
    to: LEARN_GUIDES[key]
  }))
})))

const ctaLinks = computed<ButtonProps[]>(() => [{
  label: t('home.cta.startSelling'),
  to: '/payment',
  icon: 'i-lucide-arrow-right'
}, {
  // The way in for someone who has never held a wallet: one page with the
  // wallet link, the devnet switch and the free starter tokens. It sits on the
  // test-mode card, because that is where the page says the money is free.
  label: t('home.cta.startCta'),
  to: '/start',
  icon: 'i-lucide-rocket',
  color: 'neutral',
  variant: 'subtle'
}, {
  // The card says the money is practice money; this is where a wallet that is
  // still on mainnet learns how to change that.
  label: t('home.cta.devnetGuide'),
  to: '/learn/switch-to-devnet',
  icon: 'i-lucide-network',
  color: 'neutral',
  variant: 'ghost'
}])
</script>

<template>
  <div>
    <section
      class="relative isolate flex min-h-[calc(100svh-var(--ui-header-height))] flex-col items-center justify-center overflow-hidden px-4 py-16"
    >
      <div
        aria-hidden="true"
        class="pointer-events-none absolute inset-0 -z-10"
        style="background: radial-gradient(58% 46% at 50% 0%, color-mix(in oklab, var(--ui-primary) 16%, transparent), transparent 72%)"
      />
      <div
        aria-hidden="true"
        class="pointer-events-none absolute inset-0 -z-10 opacity-40"
        style="background-image: linear-gradient(to right, var(--ui-border) 1px, transparent 1px), linear-gradient(to bottom, var(--ui-border) 1px, transparent 1px); background-size: 56px 56px; mask-image: radial-gradient(60% 50% at 50% 15%, #000, transparent 75%); -webkit-mask-image: radial-gradient(60% 50% at 50% 15%, #000, transparent 75%)"
      />

      <div class="mx-auto flex w-full max-w-3xl flex-col items-center gap-6 text-center">
        <h1 class="text-4xl font-bold tracking-tight text-balance sm:text-5xl lg:text-6xl">
          {{ $t('home.hero.title') }}
        </h1>
        <p class="text-muted max-w-2xl text-lg text-pretty sm:text-xl">
          {{ $t('home.hero.description') }}
        </p>

        <div class="flex flex-wrap items-center justify-center gap-3">
          <UButton
            v-for="link in heroLinks"
            :key="String(link.to)"
            v-bind="link"
          />
        </div>

        <div class="flex flex-wrap items-center justify-center gap-2">
          <UBadge
            v-for="proof in proofs"
            :key="proof.key"
            :icon="proof.icon"
            :label="proof.label"
            color="neutral"
            variant="outline"
            size="md"
          />
        </div>
      </div>

      <HeroPaymentCard class="mt-10 w-full max-w-sm sm:mt-12" />
    </section>

    <UPageSection>
      <WaitlistCard />
    </UPageSection>

    <UPageSection
      id="how"
      :title="$t('home.how.title')"
      :description="$t('home.how.description')"
      :features="steps"
    />

    <UPageSection
      id="faq"
      :title="$t('home.faq.title')"
      :description="$t('home.faq.description')"
    >
      <UAccordion
        :items="faqs"
        type="multiple"
        :ui="{
          trigger: 'text-lg font-semibold px-4 py-4 sm:px-0 sm:py-3.5',
          body: 'text-base px-4 pb-4 sm:px-0 sm:pb-3.5'
        }"
        class="mx-auto max-w-3xl"
      >
        <template #body="{ item }">
          <p>{{ item.content }}</p>
          <div
            v-if="item.links?.length"
            class="mt-3 flex flex-wrap items-center gap-x-4 gap-y-1 text-sm"
          >
            <ULink
              v-for="link in item.links"
              :key="link.to"
              :to="link.to"
              class="text-primary inline-flex items-center gap-1"
            >
              {{ link.label }}
              <UIcon
                name="i-lucide-arrow-right"
                class="size-3.5"
              />
            </ULink>
          </div>
        </template>
      </UAccordion>
    </UPageSection>

    <UPageSection>
      <UPageCTA
        :title="ctaTitle"
        :description="$t('home.cta.description')"
        variant="subtle"
        :links="ctaLinks"
      />
    </UPageSection>
  </div>
</template>
