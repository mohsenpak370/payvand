<script setup lang="ts">
const { t } = useI18n()

useSeoMeta({ title: t('profile.metaTitle') })
definePageMeta({ middleware: 'auth' })

const {
  user,
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
  rows,
  hasBudget,
  needsTopUp,
  topUpCurrencies,
  rolledOver,
  totalRemainingFiat,
  displayCurrency,
  budgetLoading,
  budgetError,
  transactions,
  txLoading,
  txFailure,
  refreshTransactions
} = useProfilePage()

/** The budget page owns the signature, so the profile's nudge hands over to it. */
function openBudget() {
  return navigateTo('/budget')
}
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
        :rows="rows"
        :has-budget="hasBudget"
        :total-fiat="totalRemainingFiat"
        :display-currency="displayCurrency"
        :loading="budgetLoading"
        :error="budgetError"
      />

      <AllowanceTopUpBanner
        v-if="needsTopUp"
        :rolled-over="rolledOver"
        :currencies="topUpCurrencies"
        :action-label="$t('allowance.topUpOnBudget')"
        @top-up="openBudget"
      />

      <!-- The three figures first: what the history adds up to, before the rows
           it adds up. -->
      <ProfileFlowCard
        :items="transactions?.items ?? []"
        :display-currency="displayCurrency"
        :loading="txLoading"
      />

      <ProfileTransactionsCard
        :linked-wallet="linkedWallet"
        :transactions="transactions"
        :loading="txLoading"
        :failure="txFailure"
        @refresh="() => refreshTransactions(true)"
      />
    </div>
  </UPageSection>
</template>
