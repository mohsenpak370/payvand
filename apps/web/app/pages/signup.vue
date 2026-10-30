<script setup lang="ts">
import { MIN_PASSWORD_LENGTH } from '~/composables/useAuth'

useSeoMeta({ title: 'Create account — Payvand' })

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
    failure.value = 'Enter your email address.'
    return
  }
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
    await signup(email.value, password.value)
    // Brand-new accounts start on the profile, where the wallet gets
    // connected — unless the visitor came from a specific page.
    if (explicitNext.value) {
      await router.replace(nextPath.value)
    } else {
      await router.replace({ path: '/profile', query: { welcome: '1' } })
    }
  } catch (error) {
    failure.value = apiErrorMessage(error, 'Could not create the account.')
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <UPageSection
    title="Create your account"
    description="Email and password for now — right after signup we take you to your profile to connect a Solana wallet."
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

          <UFormField
            :label="`Password (at least ${MIN_PASSWORD_LENGTH} characters)`"
          >
            <UInput
              v-model="password"
              type="password"
              name="new-password"
              autocomplete="new-password"
              class="w-full"
              placeholder="Choose a password"
            />
          </UFormField>

          <UFormField label="Confirm password">
            <UInput
              v-model="confirm"
              type="password"
              name="confirm-password"
              autocomplete="new-password"
              class="w-full"
              placeholder="Repeat it"
            />
          </UFormField>

          <UAlert
            v-if="failure"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            title="Could not create the account"
            :description="failure"
          />

          <UButton
            type="submit"
            label="Create account"
            icon="i-lucide-user-plus"
            :loading="busy"
            block
          />

          <p class="text-center text-sm text-muted">
            Already have an account?
            <NuxtLink
              :to="{ path: '/signin', query: { next: nextPath } }"
              class="text-primary font-medium"
            >
              Sign in
            </NuxtLink>
          </p>
        </form>
      </UCard>
    </div>
  </UPageSection>
</template>
