<script setup lang="ts">
/**
 * The wallet card: what is linked, what is connected, and the one-signature
 * action that joins them. The linking workflow lives in useProfileWallet; this
 * card only shows its state and passes the button presses up.
 */
defineProps<{
  linkedWallet: string | null
  showWelcome: boolean
  switchNeeded: boolean
  connected: boolean
  publicKey: string | null
  canSignMessage: boolean
  connecting: boolean
  signing: boolean
  linking: boolean
  linkFailure: string
  unlinkFailure: string
  unlinking: boolean
}>()

const emit = defineEmits<{ link: [], unlink: [] }>()
</script>

<template>
  <UCard class="min-w-0">
    <template #header>
      <div class="flex items-center justify-between gap-3">
        <div>
          <h2 class="text-base font-semibold">
            {{ $t('profile.walletTitle') }}
          </h2>
          <p class="text-sm text-muted">
            {{ $t('profile.walletDescription') }}
          </p>
        </div>
        <UBadge
          v-if="linkedWallet"
          color="success"
          variant="subtle"
        >
          {{ $t('profile.linked') }}
        </UBadge>
      </div>
    </template>

    <div class="flex flex-col gap-4">
      <UAlert
        v-if="showWelcome"
        color="info"
        variant="subtle"
        icon="i-lucide-party-popper"
        :title="$t('profile.welcomeTitle')"
        :description="$t('profile.welcomeDescription')"
      />

      <WalletConnect />

      <UAlert
        v-if="linkedWallet"
        color="success"
        variant="subtle"
        icon="i-lucide-wallet-cards"
        :title="$t('profile.linkedTitle')"
      >
        <!--
          The address has no spaces to break at, so it is allowed to break
          mid-token; otherwise it pushes the whole card wider than a phone.
        -->
        <template #description>
          {{ $t('profile.linkedSignsIn') }}
          <span class="break-all">{{ linkedWallet }}</span>
        </template>
      </UAlert>

      <UAlert
        v-else-if="connected && publicKey"
        color="info"
        variant="subtle"
        icon="i-lucide-link"
        :title="$t('profile.connectTitle')"
        :description="$t('profile.connectDescription')"
      />

      <UAlert
        v-if="linkedWallet && switchNeeded && publicKey"
        color="warning"
        variant="subtle"
        icon="i-lucide-repeat"
        :title="$t('profile.differentWalletTitle')"
        :description="$t('profile.differentWalletDescription', {
          linked: shorten(linkedWallet),
          connected: shorten(publicKey)
        })"
      />

      <UAlert
        v-if="linkFailure"
        color="error"
        variant="subtle"
        icon="i-lucide-circle-alert"
        :title="$t('profile.linkFailureTitle')"
        :description="linkFailure"
      />

      <UAlert
        v-if="unlinkFailure"
        color="error"
        variant="subtle"
        icon="i-lucide-circle-alert"
        :title="$t('profile.unlinkFailureTitle')"
        :description="unlinkFailure"
      />

      <UButton
        v-if="connected && publicKey && !linkedWallet"
        :label="canSignMessage ? $t('profile.linkThisWallet') : $t('profile.cannotSign')"
        icon="i-lucide-link"
        :loading="linking || signing || connecting"
        :disabled="!canSignMessage"
        @click="emit('link')"
      />

      <UButton
        v-if="switchNeeded && publicKey && canSignMessage"
        :label="$t('profile.switchWallet')"
        icon="i-lucide-repeat"
        :loading="linking || signing"
        @click="emit('link')"
      />

      <div
        v-if="linkedWallet"
        class="flex flex-wrap items-center justify-between gap-3 border-t border-default pt-3"
      >
        <p class="text-xs text-muted">
          {{ $t('profile.unlinkHint') }}
        </p>
        <UButton
          :label="$t('profile.unlink')"
          icon="i-lucide-unlink"
          color="neutral"
          variant="soft"
          size="sm"
          :loading="unlinking"
          @click="emit('unlink')"
        />
      </div>
    </div>
  </UCard>
</template>
