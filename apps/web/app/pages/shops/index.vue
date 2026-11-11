<script setup lang="ts">
// The shops card used to sit on the profile. It is its own page now, reached
// from the navigation, so the profile stays about the account itself.
const { t } = useI18n()

useSeoMeta({ title: t('shops.metaTitle') })
definePageMeta({ middleware: 'auth' })
</script>

<template>
  <UPageSection
    :title="$t('shops.title')"
    :description="$t('shops.description')"
  >
    <div class="mx-auto flex w-full max-w-3xl min-w-0 flex-col gap-4">
      <NuxtLink
        to="/profile"
        class="text-muted hover:text-default flex items-center gap-1.5 text-sm font-medium underline-offset-4 hover:underline"
      >
        <UIcon
          name="i-lucide-arrow-left"
          class="size-4"
        />
        {{ $t('shops.backToProfile') }}
      </NuxtLink>

      <ShopsList />
    </div>
  </UPageSection>
</template>
