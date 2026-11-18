<script setup lang="ts">
const { t } = useI18n()

useSeoMeta({ title: t('profile.metaTitle') })
definePageMeta({ middleware: 'auth' })

const {
  user,
  shopsEnabled,
  linkedWallet,
  showWelcome,
  switchNeeded,
  publicKey,
  connected,
  connecting,
  canSignMessage,
  signing,
  linkFailure,
  linking,
  unlinkFailure,
  unlinking,
  linkWalletNow,
  unlinkWalletNow,
  allowanceAddress,
  budget,
  budgetPot,
  budgetLoading,
  budgetError,
  spentThisMonth,
  remaining,
  spentPercent,
  savedCards,
  savedCount,
  savedLoading,
  savedFailure,
  refreshSavedPayments,
  transactions,
  txLoading,
  txFailure,
  refreshTransactions
} = useProfilePage()
</script>

<template>
  <UPageSection
    :title="$t('profile.title')"
    :description="$t('profile.signedInAs', { email: user?.email ?? '' })"
    :ui="{ description: 'break-words' }"
  >
    <div class="mx-auto flex w-full max-w-3xl min-w-0 flex-col gap-6">
      <div class="flex justify-end">
        <UButton
          to="/settings"
          :label="$t('profile.settings')"
          icon="i-lucide-settings"
          color="neutral"
          variant="ghost"
          size="sm"
        />
      </div>

      <ProfileWalletCard
        :linked-wallet="linkedWallet"
        :show-welcome="showWelcome"
        :switch-needed="switchNeeded"
        :connected="connected"
        :public-key="publicKey"
        :can-sign-message="canSignMessage"
        :connecting="connecting"
        :signing="signing"
        :linking="linking"
        :link-failure="linkFailure"
        :unlink-failure="unlinkFailure"
        :unlinking="unlinking"
        @link="linkWalletNow"
        @unlink="unlinkWalletNow"
      />

      <ProfileAllowanceCard
        :has-address="!!allowanceAddress"
        :budget="budget"
        :budget-pot="budgetPot"
        :loading="budgetLoading"
        :error="budgetError"
        :spent-this-month="spentThisMonth"
        :remaining="remaining"
        :spent-percent="spentPercent"
      />

      <ProfileSavedPaymentsCard
        v-if="shopsEnabled"
        :cards="savedCards"
        :count="savedCount"
        :loading="savedLoading"
        :failure="savedFailure"
        @refresh="refreshSavedPayments"
      />

      <ProfileTransactionsCard
        :linked-wallet="linkedWallet"
        :transactions="transactions"
        :loading="txLoading"
        :failure="txFailure"
        @refresh="refreshTransactions"
      />
    </div>
  </UPageSection>
</template>
