<script setup lang="ts">
import { MIN_PASSWORD_LENGTH } from '~/composables/useAuth'

const { t } = useI18n()

useSeoMeta({ title: t('signup.metaTitle') })

definePageMeta({ layout: 'auth', header: false, footer: false })

const route = useRoute()
const router = useRouter()
const { signup, ensureSession } = useAuth()

const email = ref('')
const password = ref('')
const confirm = ref('')
const failure = ref('')
const busy = ref(false)

const nextPath = computed(() => safeNextPath(route.query.next))

/** An explicit `?next=` (a deep link the visitor asked for) still wins. */
const explicitNext = computed(
  () => typeof route.query.next === 'string' && route.query.next.length > 0
)

onMounted(async () => {
  if (await ensureSession()) {
    await router.replace(nextPath.value)
  }
})

async function submit() {
  failure.value = ''
  if (!email.value.trim()) {
    failure.value = t('signup.enterEmail')
    return
  }
  if (password.value.length < MIN_PASSWORD_LENGTH) {
    failure.value = t('signup.passwordMin', { length: MIN_PASSWORD_LENGTH })
    return
  }
  if (password.value !== confirm.value) {
    failure.value = t('signup.mismatch')
    return
  }

  busy.value = true
  try {
    await signup(email.value, password.value)
    // Brand-new accounts start on the profile, where the wallet gets
    // connected — unless the visitor came from a specific page.
    if (explicitNext.value) {
      await router.replace(nextPath.value)
    } else {
      await router.replace({ path: '/profile', query: { welcome: '1' } })
    }
  } catch (error) {
    failure.value = apiErrorMessage(error, t('signup.createFailure'))
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div>
    <h1 class="text-2xl font-bold tracking-tight sm:text-3xl">
      {{ $t('signup.title') }}
    </h1>
    <p class="text-muted mt-2 text-sm leading-relaxed">
      {{ $t('signup.subtitle') }}
    </p>

    <form
      class="mt-8 flex flex-col gap-4"
      @submit.prevent="submit"
    >
      <UFormField :label="$t('signup.emailLabel')">
        <UInput
          v-model="email"
          type="email"
          name="email"
          autocomplete="email"
          class="w-full"
          :placeholder="$t('signup.emailPlaceholder')"
        />
      </UFormField>

      <UFormField
        :label="$t('signup.passwordLabel')"
        :help="$t('signup.passwordHelp', { length: MIN_PASSWORD_LENGTH })"
      >
        <UInput
          v-model="password"
          type="password"
          name="new-password"
          autocomplete="new-password"
          class="w-full"
          :placeholder="$t('signup.passwordPlaceholder')"
        />
      </UFormField>

      <UFormField :label="$t('signup.confirmLabel')">
        <UInput
          v-model="confirm"
          type="password"
          name="confirm-password"
          autocomplete="new-password"
          class="w-full"
          :placeholder="$t('signup.confirmPlaceholder')"
        />
      </UFormField>

      <UAlert
        v-if="failure"
        color="error"
        variant="subtle"
        icon="i-lucide-circle-alert"
        :description="failure"
      />

      <UButton
        type="submit"
        :label="$t('signup.submit')"
        icon="i-lucide-user-plus"
        size="lg"
        :loading="busy"
        block
      />
    </form>

    <p class="text-muted mt-6 text-center text-sm">
      {{ $t('signup.haveAccount') }}
      <NuxtLink
        :to="{ path: '/signin', query: { next: nextPath } }"
        class="text-primary font-medium"
      >
        {{ $t('signup.signIn') }}
      </NuxtLink>
    </p>
  </div>
</template>
