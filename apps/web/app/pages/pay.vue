<script setup lang="ts">
// Links handed out before the checkout had its own path carry the invoice in
// `?p=`; they are already printed and shared, so they must keep working. The
// canonical address is `/p/<token>`, which keeps the code short.
definePageMeta({ header: false, footer: false })

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

useSeoMeta({ title: 'Payment — Payvand' })
</script>

<template>
  <div class="flex min-h-[100svh] flex-col">
    <main class="flex flex-1 items-center justify-center px-5 py-10">
      <div
        v-if="embedded"
        class="text-sm text-muted"
      >
        Opening the payment…
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
          No payment in this link
        </h1>
        <p class="text-sm text-muted">
          Payment pages open from the link the seller shares. Ask them for it, or scan their QR
          code with your phone.
        </p>
        <NuxtLink
          to="/"
          class="mt-2 text-sm font-medium text-primary underline-offset-4 hover:underline"
        >
          Go to Payvand
        </NuxtLink>
      </div>
    </main>

    <footer class="border-t border-default px-5 py-5 text-center text-xs text-muted">
      Powered by
      <NuxtLink
        to="/"
        class="font-medium text-primary underline-offset-4 hover:underline"
      >
        Payvand
      </NuxtLink>
    </footer>
  </div>
</template>
