<script setup lang="ts">
const { t } = useI18n()

useSeoMeta({
  title: t('terms.metaTitle'),
  description: t('terms.metaDescription')
})

const updated = t('terms.updated')
</script>

<template>
  <UContainer class="py-8 lg:py-12">
    <UPage>
      <UPageHeader
        :title="$t('terms.title')"
        :description="$t('terms.description')"
      />

      <UPageBody class="max-w-3xl">
        <div class="space-y-10">
          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              {{ $t('terms.what.title') }}
            </h2>
            <p class="text-muted leading-relaxed">
              {{ $t('terms.what.body') }}
            </p>
            <p class="text-muted leading-relaxed">
              {{ $t('terms.what.accept') }}
            </p>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              {{ $t('terms.money.title') }}
            </h2>
            <ul class="text-muted list-disc space-y-2 ps-5 leading-relaxed">
              <li>
                {{ $t('terms.money.hold') }}
              </li>
              <li>
                {{ $t('terms.money.reverse') }}
              </li>
              <li>
                {{ $t('terms.money.seed') }}
              </li>
              <li>
                {{ $t('terms.money.budgetBefore') }}
                <NuxtLink
                  to="/privacy"
                  class="text-primary underline-offset-4 hover:underline"
                >{{ $t('terms.money.privacyLink') }}</NuxtLink>{{ $t('terms.money.budgetAfter') }}
              </li>
            </ul>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              {{ $t('terms.account.title') }}
            </h2>
            <ul class="text-muted list-disc space-y-2 ps-5 leading-relaxed">
              <li>
                {{ $t('terms.account.email') }}
              </li>
              <li>
                {{ $t('terms.account.password') }}
              </li>
              <li>
                {{ $t('terms.account.wallet') }}
              </li>
              <li>
                {{ $t('terms.account.close') }}
              </li>
              <li>
                {{ $t('terms.account.age') }}
              </li>
            </ul>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              {{ $t('terms.selling.title') }}
            </h2>
            <ul class="text-muted list-disc space-y-2 ps-5 leading-relaxed">
              <li>
                {{ $t('terms.selling.record') }}
              </li>
              <li>
                {{ $t('terms.selling.final') }}
              </li>
              <li>
                {{ $t('terms.selling.public') }}
              </li>
              <li>
                {{ $t('terms.selling.law') }}
              </li>
            </ul>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              {{ $t('terms.paying.title') }}
            </h2>
            <ul class="text-muted list-disc space-y-2 ps-5 leading-relaxed">
              <li>
                {{ $t('terms.paying.read') }}
              </li>
              <li>
                {{ $t('terms.paying.undo') }}
              </li>
              <li>
                {{ $t('terms.paying.fiat') }}
              </li>
              <li>
                {{ $t('terms.paying.trust') }}
              </li>
            </ul>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              {{ $t('terms.fees.title') }}
            </h2>
            <p class="text-muted leading-relaxed">
              {{ $t('terms.fees.body') }}
            </p>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              {{ $t('terms.testMoney.title') }}
            </h2>
            <p class="text-muted leading-relaxed">
              {{ $t('terms.testMoney.before') }} <strong class="text-default font-semibold">devnet</strong>{{ $t('terms.testMoney.after') }}
            </p>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              {{ $t('terms.fairUse.title') }}
            </h2>
            <p class="text-muted leading-relaxed">
              {{ $t('terms.fairUse.body') }}
            </p>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              {{ $t('terms.availability.title') }}
            </h2>
            <ul class="text-muted list-disc space-y-2 ps-5 leading-relaxed">
              <li>
                {{ $t('terms.availability.asIs') }}
              </li>
              <li>
                {{ $t('terms.availability.advice') }}
              </li>
              <li>
                {{ $t('terms.availability.liability') }}
              </li>
              <li>
                {{ $t('terms.availability.limits') }}
              </li>
            </ul>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              {{ $t('terms.changes.title') }}
            </h2>
            <p class="text-muted leading-relaxed">
              {{ $t('terms.changes.body') }}
            </p>
            <p class="text-muted leading-relaxed">
              {{ $t('terms.changes.contact') }}
            </p>
          </section>

          <p class="text-dimmed border-default border-t pt-6 text-sm">
            {{ $t('terms.footer', { date: updated }) }}
          </p>
        </div>
      </UPageBody>
    </UPage>
  </UContainer>
</template>
