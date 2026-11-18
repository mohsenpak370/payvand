<script setup lang="ts">
const { t } = useI18n()

useSeoMeta({ title: t('resetPassword.metaTitle') })

definePageMeta({ layout: 'blank' })

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
    failure.value = t('resetPassword.passwordMin', { length: MIN_PASSWORD_LENGTH })
    return
  }
  if (password.value !== confirm.value) {
    failure.value = t('resetPassword.mismatch')
    return
  }

  busy.value = true
  try {
    await resetPassword(token.value, password.value)
    done.value = true
  } catch (error) {
    failure.value = apiErrorMessage(error, t('resetPassword.expired'))
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <UPageSection
    :title="done ? $t('resetPassword.changedTitle') : $t('resetPassword.chooseTitle')"
    :description="
      done
        ? $t('resetPassword.changedDescription')
        : $t('resetPassword.chooseDescription')
    "
  >
    <div class="mx-auto max-w-md">
      <UCard>
        <UAlert
          v-if="!token && !done"
          color="error"
          variant="subtle"
          icon="i-lucide-circle-alert"
          :title="$t('resetPassword.invalidTitle')"
          :description="$t('resetPassword.invalidDescription')"
        >
          <template #footer>
            <UButton
              to="/forgot-password"
              :label="$t('resetPassword.requestNew')"
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
          :title="$t('resetPassword.successTitle')"
          :description="$t('resetPassword.successDescription')"
        >
          <template #footer>
            <UButton
              to="/signin"
              :label="$t('signin.submit')"
              icon="i-lucide-log-in"
            />
          </template>
        </UAlert>

        <form
          v-else
          class="flex flex-col gap-4"
          @submit.prevent="submit"
        >
          <UFormField :label="$t('resetPassword.newPasswordLabel')">
            <UInput
              v-model="password"
              type="password"
              name="new-password"
              autocomplete="new-password"
              class="w-full"
              :placeholder="$t('resetPassword.newPasswordPlaceholder')"
            />
          </UFormField>

          <UFormField :label="$t('resetPassword.confirmLabel')">
            <UInput
              v-model="confirm"
              type="password"
              name="confirm-password"
              autocomplete="new-password"
              class="w-full"
              :placeholder="$t('resetPassword.confirmPlaceholder')"
            />
          </UFormField>

          <UAlert
            v-if="failure"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            :title="$t('resetPassword.failureTitle')"
            :description="failure"
          />

          <UButton
            type="submit"
            :label="$t('resetPassword.submit')"
            icon="i-lucide-key-round"
            :loading="busy"
            block
          />
        </form>
      </UCard>
    </div>
  </UPageSection>
</template>
