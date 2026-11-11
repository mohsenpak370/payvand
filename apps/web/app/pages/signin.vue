<script setup lang="ts">
const { t } = useI18n()

useSeoMeta({ title: t('signin.metaTitle') })

definePageMeta({ layout: 'auth', header: false, footer: false })

const route = useRoute()
const router = useRouter()
const { signin, ensureSession, walletChallenge, signinWithWallet } = useAuth()
const { publicKey, connected, connecting, canConnect, connect, disconnect } = useSolanaWallet()
const { wallets, selectedWallet, selectWallet } = useSolanaWallets()
const { execute: signMessage, loading: signing } = useSolanaSignMessage()

const email = ref('')
const password = ref('')
const failure = ref('')
const busy = ref(false)
const walletBusy = ref(false)
const walletFailure = ref('')

/**
 * Wallet discovery only runs in the browser, so before mount there is
 * nothing to report about a missing extension — the note must not flash for
 * wallets that are still injecting.
 */
const walletsReady = ref(false)

const nextPath = computed(() => safeNextPath(route.query.next))

const walletAvailable = computed(() => Boolean(wallets.value.length) || connected.value)

onMounted(async () => {
  walletsReady.value = true
  if (await ensureSession()) {
    await router.replace(nextPath.value)
  }
})

function shorten(value: string) {
  return `${value.slice(0, 4)}…${value.slice(-4)}`
}

async function submit() {
  failure.value = ''
  if (!email.value.trim()) {
    failure.value = t('signin.enterEmail')
    return
  }
  if (!password.value) {
    failure.value = t('signin.enterPassword')
    return
  }

  busy.value = true
  try {
    await signin(email.value, password.value)
    await router.replace(nextPath.value)
  } catch (error) {
    failure.value = apiErrorMessage(error, t('signin.failure'))
  } finally {
    busy.value = false
  }
}

/**
 * Passwordless: one click connects the wallet if needed, then it signs a
 * server-issued challenge. Only wallets already linked from the profile
 * page open a session — otherwise the API says so.
 */
async function submitWallet() {
  walletFailure.value = ''

  walletBusy.value = true
  try {
    if (!connected.value || !publicKey.value) {
      const wallet = selectedWallet.value ?? wallets.value[0]
      if (!wallet) {
        walletFailure.value = t('signin.noWalletFound')
        return
      }
      selectWallet(wallet)
      await nextTick()
      if (canConnect.value) await connect()
      await nextTick()
    }

    const address = publicKey.value
    if (!address) {
      walletFailure.value = t('signin.walletNotConnected')
      return
    }

    const issued = await walletChallenge(address, 'signin')
    const result = await signMessage(new TextEncoder().encode(issued.message))
    await signinWithWallet(address, base58Encode(result.signature))
    await router.replace(nextPath.value)
  } catch (error) {
    const code = (error as { code?: string } | null)?.code
    walletFailure.value
      = code === 'USER_REJECTED'
        ? t('signin.cancelled')
        : apiErrorMessage(error, t('signin.declined'))
  } finally {
    walletBusy.value = false
  }
}
</script>

<template>
  <div>
    <h1 class="text-2xl font-bold tracking-tight sm:text-3xl">
      {{ $t('signin.title') }}
    </h1>
    <p class="mt-2 text-sm leading-relaxed text-muted">
      {{ $t('signin.subtitle') }}
    </p>

    <form
      class="mt-8 flex flex-col gap-4"
      @submit.prevent="submit"
    >
      <UFormField :label="$t('signin.emailLabel')">
        <UInput
          v-model="email"
          type="email"
          name="email"
          autocomplete="email"
          class="w-full"
          :placeholder="$t('signin.emailPlaceholder')"
        />
      </UFormField>

      <UFormField :label="$t('signin.passwordLabel')">
        <UInput
          v-model="password"
          type="password"
          name="password"
          autocomplete="current-password"
          class="w-full"
          :placeholder="$t('signin.passwordPlaceholder')"
        />
      </UFormField>

      <p class="-mt-1 text-right text-sm">
        <NuxtLink
          to="/forgot-password"
          class="text-primary font-medium"
        >
          {{ $t('signin.forgot') }}
        </NuxtLink>
      </p>

      <UAlert
        v-if="failure"
        color="error"
        variant="subtle"
        icon="i-lucide-circle-alert"
        :description="failure"
      />

      <UButton
        type="submit"
        :label="$t('signin.submit')"
        icon="i-lucide-arrow-right"
        size="lg"
        :loading="busy"
        block
      />
    </form>

    <USeparator
      :label="$t('signin.or')"
      class="my-6"
    />

    <div class="flex flex-col gap-3">
      <div
        v-if="connected && publicKey"
        class="flex items-center justify-between gap-3 rounded-lg border border-default bg-elevated/50 px-3 py-2"
      >
        <span class="flex min-w-0 items-center gap-2 text-sm">
          <UIcon
            name="i-lucide-wallet"
            class="text-primary size-4 shrink-0"
          />
          <span class="truncate font-mono">{{ shorten(publicKey) }}</span>
        </span>
        <button
          type="button"
          class="text-primary shrink-0 text-sm font-medium hover:underline"
          @click="disconnect()"
        >
          {{ $t('signin.disconnect') }}
        </button>
      </div>

      <UButton
        :label="connected && publicKey ? $t('signin.withWallet') : $t('signin.continueWithWallet')"
        icon="i-lucide-wallet"
        color="neutral"
        variant="subtle"
        size="lg"
        :loading="walletBusy || signing || connecting"
        :disabled="walletsReady && !walletAvailable"
        block
        @click="submitWallet"
      />

      <UAlert
        v-if="walletFailure"
        color="error"
        variant="subtle"
        icon="i-lucide-circle-alert"
        :description="walletFailure"
      />

      <p
        v-if="walletsReady && !wallets.length"
        class="text-muted text-xs leading-relaxed"
      >
        {{ $t('signin.noWalletLine') }}
        <a
          href="https://phantom.app"
          target="_blank"
          rel="noopener noreferrer"
          class="text-primary font-medium"
        >{{ $t('signin.installPhantom') }}</a>
        {{ $t('signin.or') }}
        <a
          href="https://solflare.io"
          target="_blank"
          rel="noopener noreferrer"
          class="text-primary font-medium"
        >{{ $t('signin.installSolflare') }}</a>{{ $t('signin.afterInstall') }}
      </p>

      <p class="text-muted text-xs leading-relaxed">
        {{ $t('signin.walletHint') }}
      </p>
    </div>

    <p class="text-muted mt-8 text-center text-sm">
      {{ $t('signin.newHere') }}
      <NuxtLink
        :to="{ path: '/signup', query: { next: nextPath } }"
        class="text-primary font-medium"
      >
        {{ $t('signin.createAccount') }}
      </NuxtLink>
    </p>
  </div>
</template>
