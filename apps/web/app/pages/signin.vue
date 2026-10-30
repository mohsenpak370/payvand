<script setup lang="ts">
useSeoMeta({ title: 'Sign in — Payvand' })

const route = useRoute()
const router = useRouter()
const { signin, ensureSession, walletChallenge, signinWithWallet } = useAuth()
const { publicKey, connected } = useSolanaWallet()
const { execute: signMessage, loading: signing } = useSolanaSignMessage()

const email = ref('')
const password = ref('')
const failure = ref('')
const busy = ref(false)
const walletBusy = ref(false)
const walletFailure = ref('')

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

/**
 * Passwordless: the wallet signs a server-issued challenge. Only works for
 * wallets already linked from the profile page — otherwise the API says so.
 */
async function submitWallet() {
  const address = publicKey.value
  if (!address) return

  walletFailure.value = ''
  walletBusy.value = true
  try {
    const issued = await walletChallenge(address, 'signin')
    const result = await signMessage(new TextEncoder().encode(issued.message))
    await signinWithWallet(address, base58Encode(result.signature))
    await router.replace(nextPath.value)
  } catch (error) {
    walletFailure.value = apiErrorMessage(
      error,
      'The wallet declined to sign, or the request expired. Try again.'
    )
  } finally {
    walletBusy.value = false
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

          <p class="-mt-2 text-right text-sm">
            <NuxtLink
              to="/forgot-password"
              class="text-primary font-medium"
            >
              Forgot password?
            </NuxtLink>
          </p>

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

        <UDivider
          label="or"
          class="my-5"
        />

        <div class="flex flex-col gap-3">
          <p class="text-center text-sm text-muted">
            Sign in with a wallet you linked from your profile.
          </p>

          <WalletConnect />

          <UAlert
            v-if="walletFailure"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            title="Could not sign in with the wallet"
            :description="walletFailure"
          />

          <UButton
            v-if="connected && publicKey"
            label="Sign in with wallet"
            icon="i-lucide-wallet"
            :loading="walletBusy || signing"
            block
            @click="submitWallet"
          />
        </div>
      </UCard>
    </div>
  </UPageSection>
</template>
