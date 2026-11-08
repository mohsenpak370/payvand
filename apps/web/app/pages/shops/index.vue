<script setup lang="ts">
// The shops card used to sit on the profile. It is its own page now, reached
// from the navigation, so the profile stays about the account itself.
useSeoMeta({ title: 'Shops — Payvand' })
definePageMeta({ middleware: 'auth' })
</script>

<template>
  <UPageSection
    title="Shops"
    description="Each shop holds the products you sell. Open one to edit its catalogue or to price a payment from it."
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
        Back to your profile
      </NuxtLink>

      <ShopsList />
    </div>
  </UPageSection>
</template>
