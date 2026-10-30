<script setup lang="ts">
useSeoMeta({ title: 'Sign in — Payvand' })

const route = useRoute()
const router = useRouter()
const { signin, ensureSession } = useAuth()

const email = ref('')
const password = ref('')
const failure = ref('')
const busy = ref(false)

const nextPath = computed(() => safeNextPath(route.query.next))

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
  if (!password.value) {
    failure.value = 'Enter your password.'
    return
  }

  busy.value = true
  try {
    await signin(email.value, password.value)
    await router.replace(nextPath.value)
  } catch (error) {
    failure.value = apiErrorMessage(error, 'Could not sign in. Check your email and password.')
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <UPageSection
    title="Sign in"
    description="Your account keeps the codes you create and the payments you make attached to you."
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

          <UFormField label="Password">
            <UInput
              v-model="password"
              type="password"
              name="password"
              autocomplete="current-password"
              class="w-full"
              placeholder="Your password"
            />
          </UFormField>

          <UAlert
            v-if="failure"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            title="Could not sign in"
            :description="failure"
          />

          <UButton
            type="submit"
            label="Sign in"
            icon="i-lucide-log-in"
            :loading="busy"
            block
          />

          <p class="text-center text-sm text-muted">
            No account yet?
            <NuxtLink
              :to="{ path: '/signup', query: { next: nextPath } }"
              class="text-primary font-medium"
            >
              Create one
            </NuxtLink>
          </p>
        </form>
      </UCard>
    </div>
  </UPageSection>
</template>
