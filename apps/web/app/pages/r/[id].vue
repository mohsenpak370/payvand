<script setup lang="ts">
// A reusable code opens here. The code is the offer, not an invoice, so the page
// asks for this visit's own invoice first and hands it to the same checkout
// every other code uses — same sheet, same sounds, same result.
definePageMeta({ layout: 'blank' })

const route = useRoute()
const { t } = useI18n()
const api = usePaymentApi()

const payload = ref('')
const error = ref('')
const loading = ref(true)

onMounted(async () => {
  try {
    const created = await api.reusableInvoice(String(route.params.id ?? ''))
    payload.value = created.qrPayload
  } catch (cause) {
    error.value = apiErrorMessage(cause, t('reusable.failureBody'))
  } finally {
    loading.value = false
  }
})

useSeoMeta({ title: t('reusable.metaTitle') })
</script>

<template>
  <PaymentCheckout
    v-if="payload"
    :source="payload"
  />

  <div
    v-else
    class="flex min-h-[100svh] items-center justify-center px-5 py-10"
  >
    <div class="w-full max-w-sm text-center">
      <template v-if="loading">
        <UIcon
          name="i-lucide-loader-circle"
          class="size-8 animate-spin text-muted"
          aria-hidden="true"
        />
        <p class="mt-3 text-sm text-muted">
          {{ $t('reusable.opening') }}
        </p>
      </template>

      <template v-else>
        <UIcon
          name="i-lucide-unlink"
          class="size-8 text-muted"
        />
        <h1 class="mt-2 text-base font-semibold">
          {{ $t('reusable.failureTitle') }}
        </h1>
        <p class="mt-1 text-sm text-muted">
          {{ error }}
        </p>
        <NuxtLink
          to="/"
          class="mt-3 inline-block text-sm font-medium text-primary underline-offset-4 hover:underline"
        >
          {{ $t('reusable.goHome') }}
        </NuxtLink>
      </template>
    </div>
  </div>
</template>
