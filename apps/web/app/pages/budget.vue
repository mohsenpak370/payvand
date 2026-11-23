<script setup lang="ts">
import { currencySymbol } from '~/composables/usePaymentApi'

const { t } = useI18n()

useSeoMeta({ title: t('budget.metaTitle') })

definePageMeta({ middleware: 'auth' })

const {
  connected,
  txLoading,
  currencies,
  activeCurrency,
  limitAmount,
  defaultLimits,
  busy,
  failure,
  failureTitle,
  success,
  view,
  foreignDevice,
  deviceKeyAddress,
  recoverableFloat,
  sweeping,
  refillNeeded,
  openCost,
  openCostMissing,
  budgetsRead,
  saveLimit,
  rotateToThisDevice,
  refill,
  closeBudget,
  recoverFloat
} = useBudgetPage()

/** Every action waits while a signature — or the quote feeding it — is in flight. */
const working = computed(() => busy.value || txLoading.value)

/** `Refill to 0.1 SOL`: the button the nudge offers on this page. */
const refillLabel = computed(() =>
  view.value
    ? t('budget.refill', {
        amount: budgetAmountLabel(activeCurrency.value, view.value.limit)
      })
    : ''
)
</script>

<template>
  <UPageSection
    :title="$t('budget.title')"
    :description="$t('budget.description')"
  >
    <UCard>
      <template #header>
        <div class="flex items-center justify-between gap-3">
          <h2 class="text-base font-semibold">
            {{ $t('budget.cardTitle') }}
          </h2>
          <WalletConnect />
        </div>
      </template>

      <div class="flex flex-col gap-4">
        <BudgetCurrencySwitch
          v-if="currencies.length > 1"
          v-model="activeCurrency"
          :currencies="currencies"
        />

        <template v-if="connected && view">
          <AllowanceTopUpBanner
            v-if="view.needsTopUp"
            :rolled-over="view.rolledOver"
            :currencies="currencySymbol(activeCurrency)"
            :action-label="refillLabel"
            @top-up="refill"
          />

          <BudgetSummary
            :currency="activeCurrency"
            :limit="view.limit"
            :spendable="view.spendable"
            :pot="view.pot"
            :session-key="view.sessionKey"
            :foreign-device="foreignDevice"
          />

          <p class="text-sm text-muted">
            <span class="font-medium text-highlighted">{{ $t('budget.whereMoney.title') }}</span>
            {{ $t('budget.whereMoney.body') }}
          </p>

          <p class="text-sm text-muted">
            <span class="font-medium text-highlighted">{{ $t('budget.keepSmall.title') }}</span>
            {{ $t('budget.keepSmall.body') }}
            <NuxtLink
              to="/learn/two-wallets"
              class="font-medium text-primary underline-offset-4 hover:underline"
            >
              {{ $t('budget.keepSmall.twoWallets') }}
            </NuxtLink>.
          </p>

          <BudgetForeignDeviceAlert
            v-if="foreignDevice"
            :device-key-address="deviceKeyAddress"
            :busy="working"
            @take-over="rotateToThisDevice"
          />

          <BudgetLimitForm
            v-model="limitAmount"
            :currency="activeCurrency"
            :placeholder="defaultLimits[activeCurrency]"
            help
            :cost="openCost"
            :cost-missing="openCostMissing"
          />

          <BudgetAllowanceActions
            :currency="activeCurrency"
            :limit="view.limit"
            :refill-needed="refillNeeded"
            :busy="working"
            @refill="refill"
            @save="saveLimit"
            @close="closeBudget"
          />

          <p class="text-sm text-muted">
            <span class="font-medium text-highlighted">{{ $t('budget.killSwitch.title') }}</span>
            {{ $t('budget.killSwitch.body') }}
          </p>
        </template>

        <BudgetEnableForm
          v-else-if="connected"
          v-model="limitAmount"
          :currency="activeCurrency"
          :placeholder="defaultLimits[activeCurrency]"
          :cost="openCost"
          :cost-missing="openCostMissing"
          :busy="working"
          @submit="saveLimit"
        />

        <p
          v-else
          class="text-sm text-muted"
        >
          {{ $t('budget.connectPrompt') }}
        </p>

        <BudgetRecoverAlert
          v-if="connected && budgetsRead && recoverableFloat > 0 && !busy"
          :amount="recoverableFloat"
          :busy="working || sweeping"
          @recover="recoverFloat"
        />

        <BudgetStatusAlerts
          :failure="failure"
          :failure-title="failureTitle"
          :success="success"
        />
      </div>
    </UCard>
  </UPageSection>
</template>
