<script setup lang="ts">
// One shop, opened from /shops. Everything on this page belongs to the
// signed-in account: the API scopes each read and write to it.
useSeoMeta({ title: 'Shop — Payvand' })

definePageMeta({ middleware: 'auth' })

const route = useRoute()
const shopId = computed(() => String(route.params.id ?? ''))
</script>

<template>
  <UPageSection
    title="Shop"
    description="Your products, and the prices the payment page charges."
  >
    <div class="mx-auto flex w-full max-w-3xl min-w-0 flex-col gap-4">
      <NuxtLink
        to="/shops"
        class="flex items-center gap-1.5 text-sm font-medium text-muted underline-offset-4 hover:text-default hover:underline"
      >
        <UIcon
          name="i-lucide-arrow-left"
          class="size-4"
        />
        Back to your shops
      </NuxtLink>

      <ShopEditor :shop-id="shopId" />
    </div>
  </UPageSection>
</template>
