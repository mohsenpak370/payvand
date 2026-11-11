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
}])
</script>

<template>
  <div>
    <!--
      The opening screen is the hero and nothing else: it takes the height left
      under the header, and the page below it starts after a scroll. `svh` rather
      than `vh` so a phone's collapsing browser chrome does not push the next
      section half into view.
    -->
    <UPageHero
      class="flex min-h-[calc(100svh-var(--ui-header-height))] flex-col justify-center"
      :title="$t('home.hero.title')"
      :ui="{ title: 'leading-tight' }"
      :description="$t('home.hero.description')"
      :links="heroLinks"
    />

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
