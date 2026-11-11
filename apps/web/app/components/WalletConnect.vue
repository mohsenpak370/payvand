<script setup lang="ts">
const { wallets, selectedWallet, selectWallet } = useSolanaWallets()
const { publicKey, connected, connecting, connect, disconnect, canConnect } = useSolanaWallet()

/**
 * vue-solana runs its autoConnect exactly once while the app boots. If the
 * extension is not answering at that instant — still injecting, locked, or
 * the wallet-discovery event raced the boot attempt — that one attempt is
 * burned and never retried, so a page reload falls back to the connect
 * buttons even though the wallet would reconnect without any popup. Give
 * the same silent connect the button uses a couple more chances.
 */
const RECONNECT_DELAYS_MS = [600, 3000]
const MAX_RESTORE_ATTEMPTS = 2
const SELECTED_WALLET_STORAGE_KEY = 'vue-solana:selected-wallet'

let restoreAttempts = 0
let restoreGaveUp = false
let userDisconnected = false
let restoreTimers: ReturnType<typeof setTimeout>[] = []

/** The name vue-solana persisted for the last selected wallet, if any. */
function storedWalletName(): string | null {
  try {
    const raw = window.localStorage.getItem(SELECTED_WALLET_STORAGE_KEY)
    if (!raw) return null
    const parsed: unknown = JSON.parse(raw)
    if (typeof parsed !== 'object' || parsed === null) return null
    const { name } = parsed as { name?: unknown }
    return typeof name === 'string' ? name : null
  } catch {
    return null
  }
}

async function restoreConnection() {
  if (restoreGaveUp || connected.value || connecting.value || userDisconnected) return
  if (restoreAttempts >= MAX_RESTORE_ATTEMPTS) return

  // The library may have restored the selection but failed its one
  // boot-time connect. Re-selecting also heals a persisted selection
  // whose stored shape no longer matches the discovered wallet.
  if (!selectedWallet.value) {
    const name = storedWalletName()
    const match = name ? wallets.value.find(wallet => wallet.name === name) : undefined
    if (!match) return
    selectWallet(match)
    await nextTick()
  }
  if (!canConnect.value) return

  restoreAttempts += 1
  try {
    await connect()
  } catch (error) {
    console.warn('[Payvand] silent wallet reconnect failed', error)
    // The user closing the approval window means "no" — stop asking.
    if ((error as { code?: string } | null)?.code === 'USER_REJECTED') restoreGaveUp = true
  }
}

/** Disconnection is a choice: the pending restore attempts must respect it. */
async function handleDisconnect() {
  userDisconnected = true
  await disconnect()
}

onMounted(() => {
  restoreTimers = RECONNECT_DELAYS_MS.map(
    delay => setTimeout(() => { void restoreConnection() }, delay)
  )
})

// Extensions inject their wallet-standard interface at different times,
// and the library's one-shot boot attempt can fire before its wallet is
// even discoverable — so also retry whenever the list grows.
watch(wallets, () => {
  void restoreConnection()
})

onUnmounted(() => {
  for (const timer of restoreTimers) clearTimeout(timer)
})

async function pick(wallet: NonNullable<typeof selectedWallet.value>) {
  userDisconnected = false
  selectWallet(wallet)
  await nextTick()
  if (canConnect.value && !connected.value) await connect()
}

function shorten(value: string) {
  return `${value.slice(0, 4)}…${value.slice(-4)}`
}
</script>

<template>
  <div class="flex flex-wrap items-center gap-2">
    <template v-if="connected && publicKey">
      <UBadge
        color="primary"
        variant="subtle"
        size="lg"
      >
        {{ shorten(publicKey) }}
      </UBadge>
      <UButton
        :label="$t('wallet.disconnect')"
        color="neutral"
        variant="soft"
        size="sm"
        @click="handleDisconnect"
      />
    </template>

    <template v-else>
      <UButton
        v-for="wallet in wallets"
        :key="wallet.name"
        :label="wallet.name"
        color="primary"
        variant="soft"
        size="sm"
        :loading="connecting && selectedWallet?.name === wallet.name"
        @click="pick(wallet)"
      />
      <p
        v-if="!wallets.length"
        class="text-sm text-muted"
      >
        {{ $t('wallet.none') }}
      </p>
    </template>
  </div>
</template>
