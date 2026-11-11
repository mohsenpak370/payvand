<script setup lang="ts">
const { t } = useI18n()

useSeoMeta({
  title: t('privacy.metaTitle'),
  description: t('privacy.metaDescription')
})

const updated = t('privacy.updated')
</script>

<template>
  <UContainer class="py-8 lg:py-12">
    <UPage>
      <UPageHeader
        :title="$t('privacy.title')"
        :description="$t('privacy.description')"
      />

      <UPageBody class="max-w-3xl">
        <div class="space-y-10">
          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              {{ $t('privacy.short.title') }}
            </h2>
            <ul class="text-muted list-disc space-y-2 ps-5 leading-relaxed">
              <li>
                {{ $t('privacy.short.email') }}
              </li>
              <li>
                {{ $t('privacy.short.keep') }}
              </li>
              <li>
                {{ $t('privacy.short.noAds') }}
              </li>
              <li>
                {{ $t('privacy.short.keys') }}
              </li>
              <li>
                {{ $t('privacy.short.chain') }}
              </li>
            </ul>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              {{ $t('privacy.store.title') }}
            </h2>
            <ul class="text-muted list-disc space-y-2 ps-5 leading-relaxed">
              <li>
                <strong class="text-default font-semibold">{{ $t('privacy.store.accountLead') }}</strong>
                {{ $t('privacy.store.accountBody') }}
              </li>
              <li>
                <strong class="text-default font-semibold">{{ $t('privacy.store.sessionsLead') }}</strong>
                {{ $t('privacy.store.sessionsBody') }}
              </li>
              <li>
                <strong class="text-default font-semibold">{{ $t('privacy.store.createdLead') }}</strong>
                {{ $t('privacy.store.createdBody') }}
              </li>
              <li>
                <strong class="text-default font-semibold">{{ $t('privacy.store.resetsLead') }}</strong>
                {{ $t('privacy.store.resetsBody') }}
              </li>
              <li>
                <strong class="text-default font-semibold">{{ $t('privacy.store.waitlistLead') }}</strong>
                {{ $t('privacy.store.waitlistBody') }}
              </li>
              <li>
                <strong class="text-default font-semibold">{{ $t('privacy.store.logsLead') }}</strong>
                {{ $t('privacy.store.logsBody') }}
              </li>
            </ul>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              {{ $t('privacy.visible.title') }}
            </h2>
            <ul class="text-muted list-disc space-y-2 ps-5 leading-relaxed">
              <li>
                <strong class="text-default font-semibold">{{ $t('privacy.visible.codeLead') }}</strong>
                {{ $t('privacy.visible.codeBody') }}
              </li>
              <li>
                <strong class="text-default font-semibold">{{ $t('privacy.visible.shopLead') }}</strong>
                {{ $t('privacy.visible.shopBody') }}
              </li>
              <li>
                <strong class="text-default font-semibold">{{ $t('privacy.visible.chainLead') }}</strong>
                {{ $t('privacy.visible.chainBody') }}
              </li>
              <li>
                {{ $t('privacy.visible.email') }}
              </li>
            </ul>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              {{ $t('privacy.keys.title') }}
            </h2>
            <ul class="text-muted list-disc space-y-2 ps-5 leading-relaxed">
              <li>
                {{ $t('privacy.keys.real') }}
              </li>
              <li>
                {{ $t('privacy.keys.budget') }}
              </li>
              <li>
                {{ $t('privacy.keys.never') }}
              </li>
            </ul>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              {{ $t('privacy.browser.title') }}
            </h2>
            <ul class="text-muted list-disc space-y-2 ps-5 leading-relaxed">
              <li>
                <strong class="text-default font-semibold">{{ $t('privacy.browser.localLead') }}</strong>
                {{ $t('privacy.browser.localBody') }}
              </li>
              <li>
                <strong class="text-default font-semibold">{{ $t('privacy.browser.indexedLead') }}</strong>
                {{ $t('privacy.browser.indexedBody') }}
              </li>
              <li>
                <strong class="text-default font-semibold">{{ $t('privacy.browser.cacheLead') }}</strong>
                {{ $t('privacy.browser.cacheBody') }}
              </li>
              <li>
                {{ $t('privacy.browser.none') }}
              </li>
            </ul>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              {{ $t('privacy.others.title') }}
            </h2>
            <ul class="text-muted list-disc space-y-2 ps-5 leading-relaxed">
              <li>
                {{ $t('privacy.others.hosting') }}
              </li>
              <li>
                {{ $t('privacy.others.db') }}
              </li>
              <li>
                {{ $t('privacy.others.solana') }}
              </li>
              <li>
                {{ $t('privacy.others.email') }}
              </li>
            </ul>
            <p class="text-muted leading-relaxed">
              {{ $t('privacy.others.body') }}
            </p>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              {{ $t('privacy.retention.title') }}
            </h2>
            <ul class="text-muted list-disc space-y-2 ps-5 leading-relaxed">
              <li>{{ $t('privacy.retention.account') }}</li>
              <li>{{ $t('privacy.retention.sessions') }}</li>
              <li>{{ $t('privacy.retention.resets') }}</li>
              <li>{{ $t('privacy.retention.waitlist') }}</li>
              <li>
                {{ $t('privacy.retention.logs') }}
              </li>
              <li>
                {{ $t('privacy.retention.chain') }}
              </li>
            </ul>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              {{ $t('privacy.deletion.title') }}
            </h2>
            <p class="text-muted leading-relaxed">
              {{ $t('privacy.deletion.body') }}
            </p>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              {{ $t('privacy.children.title') }}
            </h2>
            <p class="text-muted leading-relaxed">
              {{ $t('privacy.children.body') }}
            </p>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              {{ $t('privacy.changes.title') }}
            </h2>
            <p class="text-muted leading-relaxed">
              {{ $t('privacy.changes.body') }}
            </p>
            <p class="text-muted leading-relaxed">
              {{ $t('privacy.changes.contact') }}
            </p>
          </section>

          <p class="text-dimmed border-default border-t pt-6 text-sm">
            {{ $t('privacy.footer', { date: updated }) }}
            <NuxtLink
              to="/terms"
              class="text-primary underline-offset-4 hover:underline"
            >{{ $t('privacy.termsLink') }}</NuxtLink>{{ $t('privacy.footerAfter') }}
          </p>
        </div>
      </UPageBody>
    </UPage>
  </UContainer>
</template>
