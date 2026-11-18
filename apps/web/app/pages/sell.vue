<script setup lang="ts">
const { t } = useI18n()

useSeoMeta({ title: t('sell.metaTitle') })

definePageMeta({ middleware: 'auth' })

const sell = useSellPage()
</script>

<template>
  <UPageSection
    :title="t('sell.pageTitle')"
    :description="t('sell.pageDescription')"
  >
    <div class="grid gap-6 lg:grid-cols-2">
      <SellInvoiceForm :sell="sell" />
      <SellPaymentResult :sell="sell" />
      <SellDiscountManager :sell="sell" />
    </div>

    <SellPresentDialog :sell="sell" />
  </UPageSection>
</template>
