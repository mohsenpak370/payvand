<script setup lang="ts">
const { t } = useI18n()

useSeoMeta({ title: t('forgotPassword.metaTitle') })

definePageMeta({ layout: 'auth', header: false, footer: false })

const { forgotPassword } = useAuth()

const email = ref('')
const sent = ref('')
const failure = ref('')
const busy = ref(false)

async function submit() {
  failure.value = ''
  if (!email.value.trim()) {
    failure.value = t('forgotPassword.enterEmail')
    return
  }

  busy.value = true
  try {
    // The API answers identically for every address — including ones with
    // no account — so "sent" only ever shows the server's generic message.
    sent.value = await forgotPassword(email.value)
  } catch (error) {
    failure.value = apiErrorMessage(error, t('forgotPassword.failure'))
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div>
    <h1 class="text-2xl font-bold tracking-tight sm:text-3xl">
      {{ $t('forgotPassword.title') }}
    </h1>
    <p class="text-muted mt-2 text-sm leading-relaxed">
      {{ $t('forgotPassword.subtitle') }}
    </p>

    <form
      class="mt-8 flex flex-col gap-4"
      @submit.prevent="submit"
    >
      <UFormField :label="$t('forgotPassword.emailLabel')">
        <UInput
          v-model="email"
          type="email"
          name="email"
          autocomplete="email"
          class="w-full"
          :placeholder="$t('forgotPassword.emailPlaceholder')"
        />
      </UFormField>

      <UAlert
        v-if="sent"
        color="success"
        variant="subtle"
        icon="i-lucide-mail-check"
        :title="$t('forgotPassword.sentTitle')"
        :description="sent"
      />

      <UAlert
        v-if="failure"
        color="error"
        variant="subtle"
        icon="i-lucide-circle-alert"
        :description="failure"
      />

      <UButton
        type="submit"
        :label="$t('forgotPassword.submit')"
        icon="i-lucide-send"
        size="lg"
        :loading="busy"
        :disabled="!!sent"
        block
      />
    </form>

    <p class="text-muted mt-8 text-center text-sm">
      {{ $t('forgotPassword.remembered') }}
      <NuxtLink
        to="/signin"
        class="text-primary font-medium"
      >
        {{ $t('forgotPassword.backToSignIn') }}
      </NuxtLink>
    </p>
  </div>
</template>
