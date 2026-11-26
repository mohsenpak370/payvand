<script setup lang="ts">
const { t } = useI18n()

useSeoMeta({ title: t('sell.metaTitle') })

definePageMeta({ middleware: 'auth' })

/**
 * One payment, one price. Not everyone charging for something is selling from a
 * catalogue, so this page carries nothing from the shop feature: no menus, no
 * codes, no card designs. Those belong to a shop's own payment page, where the
 * basket is the point.
 */
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
    </div>

    <SellPresentDialog :sell="sell" />
  </UPageSection>
</template>
