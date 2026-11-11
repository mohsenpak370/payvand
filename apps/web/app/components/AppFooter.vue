<script setup lang="ts">
const { t } = useI18n()
const { version } = useRuntimeConfig().public

const columns = computed(() => [
  {
    label: t('footer.groups.product'),
    children: [
      { label: t('footer.links.createPayment'), to: '/sell' },
      { label: t('footer.links.scanAndPay'), to: '/scan' },
      { label: t('footer.links.learn'), to: '/learn' }
    ]
  },
  {
    label: t('footer.groups.legal'),
    children: [
      { label: t('footer.links.terms'), to: '/terms' },
      { label: t('footer.links.privacy'), to: '/privacy' }
    ]
  }
])
</script>

<template>
  <UFooter>
    <template #top>
      <UContainer>
        <UFooterColumns :columns="columns">
          <template #left>
            <div class="max-w-sm">
              <NuxtLink
                to="/"
                :aria-label="$t('app.brandHome')"
                class="focus-visible:outline-3 outline-primary/25 inline-flex items-center gap-2 rounded-md"
              >
                <img
                  src="/logo.png"
                  alt=""
                  width="356"
                  height="368"
                  class="h-8 w-auto shrink-0"
                >
                <span class="relative text-xl font-bold">
                  <span class="text-primary">{{ $t('app.brandWord.lead') }}</span>{{ $t('app.brandWord.rest') }}
                </span>
              </NuxtLink>

              <p class="text-muted mt-4 pr-4 text-sm leading-relaxed">
                {{ $t('footer.tagline') }}
              </p>
            </div>
          </template>
        </UFooterColumns>
      </UContainer>
    </template>

    <template #left>
      <p class="text-muted text-sm">
        {{ $t('footer.copyright', { year: new Date().getFullYear(), version: `v${version}` }) }}
      </p>
    </template>

    <template #right>
      <UButton
        to="https://solana.com/docs"
        target="_blank"
        icon="i-lucide-book-open"
        :label="$t('footer.solanaDocs')"
        color="neutral"
        variant="ghost"
        size="sm"
      />
    </template>
  </UFooter>
</template>
