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
  to: '/sell',
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

// What the hero promises, in the four things a shop owner actually cares
// about: no extra hardware, no monthly bill, nothing for the customer to
// install, and the money is theirs.
const proofs = computed(() => [
  { key: 'noReader', icon: 'i-lucide-credit-card', label: t('home.hero.proof.noReader') },
  { key: 'noMarkup', icon: 'i-lucide-receipt-text', label: t('home.hero.proof.noMarkup') },
  { key: 'noInstall', icon: 'i-lucide-smartphone', label: t('home.hero.proof.noInstall') },
  { key: 'ownWallet', icon: 'i-lucide-wallet', label: t('home.hero.proof.ownWallet') }
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

const ctaLinks = computed<ButtonProps[]>(() => [{
  label: t('home.cta.startSelling'),
  to: '/sell',
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
    <!--
      The opening screen: the pitch, what it promises, and one picture of the
      result. `svh` rather than `vh` so a phone's collapsing browser chrome does
      not push the next section half into view.
    -->
    <section
      class="relative isolate flex min-h-[calc(100svh-var(--ui-header-height))] flex-col items-center justify-center overflow-hidden px-4 py-16"
    >
      <!--
        The two texture layers are decoration and only that: a soft primary glow
        behind the title, and a faint grid that fades out before it reaches the
        copy, so the background reads as deliberate rather than as empty space.
        Both are `pointer-events-none` and hidden from assistive tech.
      -->
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

        <ul class="text-muted flex flex-wrap items-center justify-center gap-x-6 gap-y-2 text-sm">
          <li
            v-for="proof in proofs"
            :key="proof.key"
            class="flex items-center gap-1.5"
          >
            <UIcon
              :name="proof.icon"
              class="text-primary size-4"
            />
            {{ proof.label }}
          </li>
        </ul>
      </div>

      <!-- The receipt gets its own row under the pitch: on a wide screen it is
           the one thing on the page that shows what a payment looks like. -->
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
