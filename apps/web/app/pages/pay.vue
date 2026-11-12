<script setup lang="ts">
// Links handed out before the checkout had its own path carry the invoice in
// `?p=`; they are already printed and shared, so they must keep working. The
// canonical address is `/p/<token>`, which keeps the code short.
definePageMeta({ layout: 'blank' })

const route = useRoute()
const embedded = computed(() => {
  const value = route.query.p
  return typeof value === 'string' ? value : ''
})

onMounted(async () => {
  if (embedded.value) {
    await navigateTo(paymentLinkPath(embedded.value), { replace: true })
  }
})

const { t } = useI18n()

useSeoMeta({ title: t('pay.metaTitle') })
</script>

<template>
  <div class="flex min-h-[100svh] flex-col">
    <main class="flex flex-1 items-center justify-center px-5 py-10">
      <div
        v-if="embedded"
        class="text-sm text-muted"
      >
        {{ $t('pay.opening') }}
      </div>

      <div
        v-else
        class="flex max-w-sm flex-col items-center gap-2 text-center"
      >
        <UIcon
          name="i-lucide-qr-code"
          class="size-8 text-muted"
        />
        <h1 class="text-base font-semibold">
          {{ $t('pay.noPaymentTitle') }}
        </h1>
        <p class="text-sm text-muted">
          {{ $t('pay.noPaymentBody') }}
        </p>
        <NuxtLink
          to="/"
          class="mt-2 text-sm font-medium text-primary underline-offset-4 hover:underline"
        >
          {{ $t('checkout.goHome') }}
        </NuxtLink>
      </div>
    </main>

    <footer class="border-t border-default px-5 py-5 text-center text-xs text-muted">
      {{ $t('checkout.poweredBy') }}
      <NuxtLink
        to="/"
        class="font-medium text-primary underline-offset-4 hover:underline"
      >
        {{ $t('app.brandWord.lead') }}{{ $t('app.brandWord.rest') }}
      </NuxtLink>
    </footer>
  </div>
</template>
