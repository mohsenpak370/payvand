<script setup lang="ts">
// One shop, opened from /shops. Everything on this page belongs to the
// signed-in account: the API scopes each read and write to it.
const { t } = useI18n()

useSeoMeta({ title: t('shops.shopMetaTitle') })

definePageMeta({ middleware: 'auth' })

const route = useRoute()
const shopId = computed(() => String(route.params.id ?? ''))
</script>

<template>
  <UPageSection
    :title="$t('shops.shopTitle')"
    :description="$t('shops.shopDescription')"
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
        {{ $t('shops.backToShops') }}
      </NuxtLink>

      <ShopEditor :shop-id="shopId" />

      <!-- A code priced from this shop's menu, made where the menu lives. -->
      <ShopPayment
        :key="shopId"
        :shop-id="shopId"
      />

      <!-- The links this shop keeps, next to the products they price. -->
      <ShopCodes
        :key="shopId"
        :shop-id="shopId"
      />
    </div>
  </UPageSection>
</template>
