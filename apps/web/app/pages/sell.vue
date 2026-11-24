<script setup lang="ts">
const { t } = useI18n()

useSeoMeta({ title: t('sell.metaTitle') })

definePageMeta({ middleware: 'auth' })

const sell = useSellPage()
// `sell` is a plain object of refs, so `sell.discountAllowed` in the template
// is the ref itself — always truthy, which is why the discount card used to
// sit on the page whatever the pricing mode was. Destructured, they become
// setup bindings, and the template unwraps those to the real values.
const { discountAllowed, shopsActivated } = sell
</script>

<template>
  <UPageSection
    :title="t('sell.pageTitle')"
    :description="t('sell.pageDescription')"
  >
    <div class="grid gap-6 lg:grid-cols-2">
      <SellInvoiceForm :sell="sell" />
      <SellPaymentResult :sell="sell" />
      <!-- Codes are part of what a shop subscription buys, so the card waits
           for the shop feature to be switched on *and* active; a typed price
           or an empty tray has nothing for a percentage to price either. -->
      <SellDiscountManager
        v-if="discountAllowed && shopsActivated"
        :sell="sell"
      />
    </div>

    <SellPresentDialog :sell="sell" />
  </UPageSection>
</template>
