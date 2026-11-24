<script setup lang="ts">
import type { SellPageState } from '~/composables/useSellPage'
import { cardDesignSwatch } from '~/utils/cardDesigns'

const props = defineProps<{
  sell: SellPageState
}>()

const { t } = useI18n()

const {
  cluster,
  shopsEnabled,
  hasProducts,
  shops,
  cart,
  cartActive,
  sellingFromShops,
  discountAllowed,
  canInviteBuyerCodes,
  priceMode,
  priceModeItems,
  priceInput,
  priceCurrency,
  currencyItems,
  calculatorOpen,
  design,
  designOptions,
  name,
  description,
  memo,
  detailsOpen,
  discounts,
  allowDiscounts,
  discountCode,
  discountOptions,
  selectedDiscount,
  discountedPrice,
  amountHelp,
  netQuote,
  canCreate,
  busy,
  failure,
  priceSummary,
  applyAmount,
  createPayment
} = props.sell
</script>

<template>
  <UCard>
    <template #header>
      <div class="flex items-center justify-between gap-3">
        <h2 class="text-base font-semibold">
          {{ t('sell.invoiceHeading') }}
        </h2>
        <WalletConnect />
      </div>
    </template>

    <div class="flex flex-col gap-4">
      <UFormField
        v-if="shopsEnabled && hasProducts"
        :label="t('sell.pricingLabel')"
      >
        <URadioGroup
          v-model="priceMode"
          :items="priceModeItems"
        />
      </UFormField>

      <ProductPicker
        v-if="shopsEnabled && sellingFromShops"
        v-model="cart"
        :shops="shops"
      />

      <!--
        One field for the price and the currency it is charged in: the
        dropdown is not a display unit, it decides what the code charges.
      -->
      <UFormField
        v-if="!sellingFromShops"
        :label="t('sell.priceLabel')"
        :help="amountHelp"
      >
        <div class="flex items-stretch gap-2">
          <UInput
            v-model="priceInput"
            type="number"
            min="0"
            :step="priceCurrency === 'sol' ? '0.001' : '0.01'"
            class="min-w-0 flex-1"
            :placeholder="priceCurrency === 'sol' ? t('sell.pricePlaceholderSol') : t('sell.pricePlaceholderToken')"
            :disabled="cartActive"
          />
          <USelect
            v-model="priceCurrency"
            :items="currencyItems"
            class="w-24"
            :aria-label="t('sell.currencyAria')"
            :disabled="cartActive"
          />
          <UButton
            icon="i-lucide-calculator"
            color="neutral"
            variant="outline"
            :aria-label="t('sell.calculatorAria')"
            :disabled="cartActive"
            @click="calculatorOpen = true"
          />
        </div>
      </UFormField>

      <!-- The split, before the code exists: the price minus the fee is
           what the seller keeps, and the buyer pays exactly the price. -->
      <InvoiceSplitNote
        :costs="netQuote"
        class="-mt-2"
      />

      <!-- A code prices a basket, so it appears only for a payment built from
           shop products. A shop's own payment page attaches one to a typed
           price itself. -->
      <UFormField
        v-if="discounts.length && discountAllowed"
        :label="t('sell.discountLabel')"
        :help="selectedDiscount
          ? t('sell.discountHelpSelected', { price: discountedPrice, percent: selectedDiscount.percent_off })
          : t('sell.discountHelp')"
      >
        <USelect
          v-model="discountCode"
          :items="discountOptions"
          class="w-full"
        />
      </UFormField>

      <!-- Only a payment built from products can offer the buyer a code, so the
           switch that invites one appears with the cart and nowhere else. -->
      <UFormField
        v-if="discounts.length && canInviteBuyerCodes"
        :label="t('sell.allowDiscountsLabel')"
        :help="t('sell.allowDiscountsHelp')"
      >
        <USwitch v-model="allowDiscounts" />
      </UFormField>

      <UFormField
        :label="t('sell.nameLabel')"
        :help="t('sell.nameHelp')"
      >
        <UInput
          v-model="name"
          class="w-full"
          :placeholder="t('sell.namePlaceholder')"
          :maxlength="64"
        />
      </UFormField>

      <!-- Two optional text fields: the buyer's detail line and the on-chain
           note. Folded, because most codes need neither. -->
      <div class="flex flex-col gap-3">
        <UButton
          :label="detailsOpen ? t('sell.hideDetails') : t('sell.addDetails')"
          :icon="detailsOpen ? 'i-lucide-chevron-up' : 'i-lucide-chevron-down'"
          color="neutral"
          variant="ghost"
          size="xs"
          block
          :aria-expanded="detailsOpen"
          @click="detailsOpen = !detailsOpen"
        />

        <template v-if="detailsOpen">
          <UFormField
            :label="t('sell.descriptionLabel')"
            :help="t('sell.descriptionHelp')"
          >
            <UTextarea
              v-model="description"
              :rows="2"
              class="w-full"
              :placeholder="t('sell.descriptionPlaceholder')"
              :maxlength="160"
            />
          </UFormField>

          <UFormField
            :label="t('sell.memoLabel')"
            :help="t('sell.memoHelp')"
          >
            <UTextarea
              v-model="memo"
              :rows="2"
              class="w-full"
              :placeholder="t('sell.memoPlaceholder')"
              :maxlength="96"
            />
          </UFormField>
        </template>
      </div>

      <!-- The look of the page the customer opens. It is part of the code, so
           it has to be chosen here, before the invoice exists — and only for a
           payment built from shop products, where the basket is the point of
           the page. A typed price is one number and one memo. -->
      <UFormField
        v-if="sellingFromShops && cartActive"
        :label="t('sell.designLabel')"
        :help="t('sell.designHelp')"
      >
        <div
          class="flex flex-wrap gap-2"
          role="radiogroup"
          :aria-label="t('sell.designLabel')"
        >
          <button
            v-for="option in designOptions"
            :key="option.value"
            type="button"
            role="radio"
            :aria-checked="design === option.value"
            class="flex flex-col items-center gap-1 rounded-lg border p-1.5 text-xs transition-colors"
            :class="design === option.value
              ? 'border-primary text-highlighted ring-2 ring-primary/30'
              : 'border-default text-muted hover:border-primary/40'"
            @click="design = option.value"
          >
            <span
              :class="cardDesignSwatch(option.value)"
              class="size-8 rounded-md"
              aria-hidden="true"
            />
            {{ option.label }}
          </button>
        </div>
      </UFormField>

      <div class="flex items-center justify-between gap-3 text-sm text-muted">
        <span>{{ networkLabel(cluster) }}</span>
        <span class="tabular-nums">
          {{ priceSummary }}
        </span>
      </div>

      <UAlert
        v-if="failure"
        color="error"
        variant="subtle"
        icon="i-lucide-circle-alert"
        :title="t('sell.createFailureTitle')"
        :description="failure"
      />

      <UButton
        :label="t('sell.createCode')"
        icon="i-lucide-qr-code"
        :loading="busy"
        :disabled="!canCreate"
        block
        @click="createPayment()"
      />
    </div>

    <CalculatorModal
      v-model:open="calculatorOpen"
      :initial="priceInput"
      @apply="applyAmount"
    />
  </UCard>
</template>
