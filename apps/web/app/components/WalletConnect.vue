<script setup lang="ts">
const { wallets, selectedWallet, selectWallet } = useSolanaWallets()
const { publicKey, connected, connecting, connect, disconnect, canConnect } = useSolanaWallet()

async function pick(wallet: NonNullable<typeof selectedWallet.value>) {
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
        label="Disconnect"
        color="neutral"
        variant="soft"
        size="sm"
        @click="disconnect"
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
        No wallet detected. Install a Solana wallet extension and reload.
      </p>
    </template>
  </div>
</template>
