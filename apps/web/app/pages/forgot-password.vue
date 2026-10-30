<script setup lang="ts">
useSeoMeta({ title: 'Forgot password — Payvand' })

const { forgotPassword } = useAuth()

const email = ref('')
const sent = ref('')
const failure = ref('')
const busy = ref(false)

async function submit() {
  failure.value = ''
  if (!email.value.trim()) {
    failure.value = 'Enter your email address.'
    return
  }

  busy.value = true
  try {
    // The API answers identically for every address — including ones with
    // no account — so "sent" only ever shows the server's generic message.
    sent.value = await forgotPassword(email.value)
  } catch (error) {
    failure.value = apiErrorMessage(error, 'Could not send the request. Try again.')
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <UPageSection
    title="Reset your password"
    description="We'll email you a one-time link to choose a new password. The link is valid for one hour."
  >
    <div class="mx-auto max-w-md">
      <UCard>
        <form
          class="flex flex-col gap-4"
          @submit.prevent="submit"
        >
          <UFormField label="Email">
            <UInput
              v-model="email"
              type="email"
              name="email"
              autocomplete="email"
              class="w-full"
              placeholder="you@example.com"
            />
          </UFormField>

          <UAlert
            v-if="sent"
            color="success"
            variant="subtle"
            icon="i-lucide-mail-check"
            title="Check your inbox"
            :description="sent"
          />

          <UAlert
            v-if="failure"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            title="Could not send the link"
            :description="failure"
          />

          <UButton
            type="submit"
            label="Send reset link"
            icon="i-lucide-send"
            :loading="busy"
            :disabled="!!sent"
            block
          />

          <p class="text-center text-sm text-muted">
            Remembered it?
            <NuxtLink
              to="/signin"
              class="text-primary font-medium"
            >
              Back to sign in
            </NuxtLink>
          </p>
        </form>
      </UCard>
    </div>
  </UPageSection>
</template>
