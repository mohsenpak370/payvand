<script setup lang="ts">
import type { SellPageState } from '~/composables/useSellPage'

const props = defineProps<{
  sell: SellPageState
}>()

const { t } = useI18n()

const {
  discounts,
  discountsOpen,
  newCode,
  newPercent,
  codeBusy,
  codeFailure,
  canAddCode,
  codeCount,
  addDiscount,
  removeDiscount
} = props.sell
</script>

<template>
  <UCard class="lg:col-span-2">
    <template #header>
      <div class="flex items-center justify-between gap-3">
        <div>
          <h2 class="text-base font-semibold">
            {{ t('sell.discountsTitle') }}
          </h2>
          <p class="text-sm text-muted">
            {{ t('sell.discountsSubtitle') }}
          </p>
        </div>
        <UButton
          :label="discountsOpen
            ? t('sell.hide')
            : discounts.length
              ? codeCount(discounts.length)
              : t('sell.addCode')"
          :icon="discountsOpen ? 'i-lucide-chevron-up' : 'i-lucide-chevron-down'"
          color="neutral"
          variant="soft"
          size="sm"
          :aria-expanded="discountsOpen"
          @click="discountsOpen = !discountsOpen"
        />
      </div>
    </template>

    <div
      v-if="discountsOpen"
      class="flex flex-col gap-4"
    >
      <ul
        v-if="discounts.length"
        class="flex flex-wrap gap-2"
      >
        <li
          v-for="discount in discounts"
          :key="discount.id"
          class="flex items-center gap-2 rounded-full border border-default py-1 ps-3 pe-1"
        >
          <span class="text-sm font-medium">{{ discount.code }}</span>
          <UBadge
            color="primary"
            variant="subtle"
          >
            {{ t('sell.percentOff', { percent: discount.percent_off }) }}
          </UBadge>
          <UButton
            icon="i-lucide-x"
            color="error"
            variant="ghost"
            size="xs"
            square
            :aria-label="t('sell.deleteAria', { code: discount.code })"
            @click="removeDiscount(discount)"
          />
        </li>
      </ul>
      <p
        v-else
        class="text-sm text-muted"
      >
        {{ t('sell.noCodes') }}
      </p>

      <div class="flex flex-wrap items-end gap-3">
        <UFormField
          :label="t('sell.codeLabel')"
          class="min-w-40 flex-1"
        >
          <UInput
            v-model="newCode"
            class="w-full"
            :placeholder="t('sell.codePlaceholder')"
            :maxlength="32"
          />
        </UFormField>
        <UFormField
          :label="t('sell.percentLabel')"
          class="w-32"
        >
          <UInput
            v-model="newPercent"
            type="number"
            min="1"
            max="99"
            class="w-full"
            :placeholder="t('sell.percentPlaceholder')"
          />
        </UFormField>
        <UButton
          :label="t('sell.addCode')"
          icon="i-lucide-plus"
          :loading="codeBusy"
          :disabled="!canAddCode"
          @click="addDiscount()"
        />
      </div>

      <UAlert
        v-if="codeFailure"
        color="error"
        variant="subtle"
        icon="i-lucide-circle-alert"
        :title="t('sell.codeErrorTitle')"
        :description="codeFailure"
      />
    </div>
  </UCard>
</template>
