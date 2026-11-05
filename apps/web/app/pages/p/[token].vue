<script setup lang="ts">
// Every shared link and every printed code lands here: one page that shows what
// is being sold and takes the payment. The app header and footer stay out of the
// way because this sheet may be printed.
definePageMeta({ header: false, footer: false })

const route = useRoute()
const token = computed(() => String(route.params.token ?? ''))
</script>

<template>
  <PaymentCheckout :source="token" />
</template>
