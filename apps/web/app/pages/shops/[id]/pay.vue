<script setup lang="ts">
// The payment page for one shop: the sell page's whole form, priced only from
// this shop's menu, with the discount codes that belong to the seller.
const { t } = useI18n()

useSeoMeta({ title: t('shopSell.metaTitle') })

definePageMeta({ middleware: 'auth' })

const route = useRoute()
const shopId = computed(() => String(route.params.id ?? ''))

const sell = useSellPage(shopId.value)
</script>

<template>
  <UPageSection
    :title="t('shopSell.pageTitle')"
    :description="t('shopSell.pageDescription')"
  >
    <div class="mx-auto flex w-full max-w-5xl min-w-0 flex-col gap-4">
      <NuxtLink
        :to="`/shops/${shopId}`"
        class="flex items-center gap-1.5 text-sm font-medium text-muted underline-offset-4 hover:text-default hover:underline"
      >
        <UIcon
          name="i-lucide-arrow-left"
          class="size-4"
        />
        {{ $t('shopSell.backToShop') }}
      </NuxtLink>

      <div class="grid gap-6 lg:grid-cols-2">
        <SellInvoiceForm :sell="sell" />
        <SellPaymentResult :sell="sell" />
        <SellDiscountManager
          v-if="sell.discountAllowed"
          :sell="sell"
        />
      </div>

      <SellPresentDialog :sell="sell" />
    </div>
  </UPageSection>
</template>
