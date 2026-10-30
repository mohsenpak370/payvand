<script setup lang="ts">
useSeoMeta({ title: 'Choose a new password — Payvand' })

const route = useRoute()
const { resetPassword } = useAuth()

const token = computed(() =>
  typeof route.query.token === 'string' ? route.query.token : ''
)

const password = ref('')
const confirm = ref('')
const failure = ref('')
const done = ref(false)
const busy = ref(false)

async function submit() {
  failure.value = ''
  if (password.value.length < MIN_PASSWORD_LENGTH) {
    failure.value = `The password must be at least ${MIN_PASSWORD_LENGTH} characters.`
    return
  }
  if (password.value !== confirm.value) {
    failure.value = 'The two passwords do not match.'
    return
  }

  busy.value = true
  try {
    await resetPassword(token.value, password.value)
    done.value = true
  } catch (error) {
    failure.value = apiErrorMessage(
      error,
      'This reset link is invalid or has expired. Request a new one.'
    )
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <UPageSection
    :title="done ? 'Password changed' : 'Choose a new password'"
    :description="
      done
        ? 'Your previous password no longer works and every existing session was signed out.'
        : 'Pick a new password for your account. For safety, this link can only be used once.'
    "
  >
    <div class="mx-auto max-w-md">
      <UCard>
        <UAlert
          v-if="!token && !done"
          color="error"
          variant="subtle"
          icon="i-lucide-circle-alert"
          title="This link is invalid or has expired"
          description="Password reset links are single-use and valid for one hour. Request a fresh one and open it right away."
        >
          <template #footer>
            <UButton
              to="/forgot-password"
              label="Request a new link"
              icon="i-lucide-mail"
              size="sm"
            />
          </template>
        </UAlert>

        <UAlert
          v-else-if="done"
          color="success"
          variant="subtle"
          icon="i-lucide-circle-check"
          title="Your password has been changed"
          description="Sign in with your new password to continue."
        >
          <template #footer>
            <UButton
              to="/signin"
              label="Sign in"
              icon="i-lucide-log-in"
            />
          </template>
        </UAlert>

        <form
          v-else
          class="flex flex-col gap-4"
          @submit.prevent="submit"
        >
          <UFormField label="New password">
            <UInput
              v-model="password"
              type="password"
              name="new-password"
              autocomplete="new-password"
              class="w-full"
              placeholder="At least 8 characters"
            />
          </UFormField>

          <UFormField label="Confirm password">
            <UInput
              v-model="confirm"
              type="password"
              name="confirm-password"
              autocomplete="new-password"
              class="w-full"
              placeholder="Repeat the new password"
            />
          </UFormField>

          <UAlert
            v-if="failure"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            title="Could not reset the password"
            :description="failure"
          />

          <UButton
            type="submit"
            label="Save new password"
            icon="i-lucide-key-round"
            :loading="busy"
            block
          />
        </form>
      </UCard>
    </div>
  </UPageSection>
</template>
