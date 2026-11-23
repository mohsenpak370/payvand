<script setup lang="ts">
// The front door for someone who has never used a wallet: one page that gets
// them a wallet on the test network, gives them free tokens, and points at the
// guide for each idea behind the screens they will press.
const { t } = useI18n()

useSeoMeta({
  title: t('start.metaTitle'),
  description: t('start.hero.description')
})

const guides = computed(() => [
  { to: '/learn/test-money-on-devnet', label: t('start.learn.devnet') },
  { to: '/learn/what-is-a-crypto-wallet', label: t('start.learn.wallet') },
  { to: '/learn/paying-with-a-wallet', label: t('start.learn.paying') },
  { to: '/learn/two-wallets', label: t('start.learn.twoWallets') },
  { to: '/learn/stablecoins-explained', label: t('start.learn.stablecoins') },
  { to: '/learn/monthly-allowance-explained', label: t('start.learn.allowance') },
  { to: '/learn/spotting-crypto-scams', label: t('start.learn.scams') }
])
</script>

<template>
  <div>
    <UPageHero
      :headline="$t('start.hero.headline')"
      :title="$t('start.hero.title')"
      :description="$t('start.hero.description')"
      :ui="{ title: 'leading-tight' }"
    />

    <UPageSection>
      <div class="mx-auto flex w-full max-w-3xl min-w-0 flex-col gap-4">
        <!-- 1. A wallet. Solflare first because it switches networks in two
             clicks, and other wallets are named so nobody thinks this is the
             only door. -->
        <UCard class="min-w-0">
          <div class="flex flex-col gap-3">
            <h3 class="text-base font-semibold">
              {{ $t('start.wallet.title') }}
            </h3>
            <p class="text-muted text-sm leading-relaxed">
              {{ $t('start.wallet.body') }}
            </p>
            <div class="flex flex-wrap items-center gap-3">
              <UButton
                to="https://www.solflare.com/download/"
                target="_blank"
                rel="noopener noreferrer"
                :label="$t('start.wallet.action')"
                icon="i-lucide-download"
                trailing-icon="i-lucide-arrow-up-right"
              />
              <span class="text-muted text-xs">{{ $t('start.wallet.note') }}</span>
            </div>
          </div>
        </UCard>

        <!-- 2. Devnet, and how to get there in a wallet. -->
        <UCard class="min-w-0">
          <div class="flex flex-col gap-3">
            <h3 class="text-base font-semibold">
              {{ $t('start.devnet.title') }}
            </h3>
            <p class="text-muted text-sm leading-relaxed">
              {{ $t('start.devnet.body') }}
            </p>
            <div class="flex flex-col gap-2 text-sm">
              <p class="font-medium">
                {{ $t('start.devnet.solflare') }}
              </p>
              <p class="text-muted leading-relaxed">
                {{ $t('start.devnet.others') }}
              </p>
            </div>
            <UButton
              to="/learn/test-money-on-devnet"
              :label="$t('start.devnet.guide')"
              icon="i-lucide-book-open"
              trailing-icon="i-lucide-arrow-right"
              color="neutral"
              variant="soft"
              size="sm"
              class="self-start"
            />
          </div>
        </UCard>

        <!-- 3. The tokens, in the app: no address to copy, no other site. -->
        <FaucetCard />

        <!-- 4. Where to go next. -->
        <UCard class="min-w-0">
          <div class="flex flex-col gap-3">
            <h3 class="text-base font-semibold">
              {{ $t('start.next.title') }}
            </h3>
            <p class="text-muted text-sm leading-relaxed">
              {{ $t('start.next.body') }}
            </p>
            <div class="flex flex-wrap gap-2">
              <UButton
                to="/profile"
                :label="$t('profile.title')"
                icon="i-lucide-circle-user-round"
                size="sm"
              />
              <UButton
                to="/budget"
                :label="$t('start.next.allowance')"
                icon="i-lucide-shield"
                color="neutral"
                variant="soft"
                size="sm"
              />
              <UButton
                to="/scan"
                :label="$t('app.nav.scanAndPay')"
                icon="i-lucide-scan-line"
                color="neutral"
                variant="soft"
                size="sm"
              />
            </div>
          </div>
        </UCard>
      </div>
    </UPageSection>

    <UPageSection
      :title="$t('start.learn.title')"
      :description="$t('start.learn.body')"
    >
      <div class="mx-auto grid w-full max-w-3xl gap-2 sm:grid-cols-2">
        <UButton
          v-for="guide in guides"
          :key="guide.to"
          :to="guide.to"
          :label="guide.label"
          icon="i-lucide-book-open"
          trailing-icon="i-lucide-arrow-right"
          color="neutral"
          variant="soft"
          block
          class="justify-start"
        />
      </div>
    </UPageSection>
  </div>
</template>
