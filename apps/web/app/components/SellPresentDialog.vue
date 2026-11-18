<script setup lang="ts">
import type { SellPageState } from '~/composables/useSellPage'

const props = defineProps<{
  sell: SellPageState
}>()

const { t } = useI18n()

const {
  record,
  qrPayload,
  presentOpen,
  paidAmountLabel
} = props.sell
</script>

<template>
  <!--
    The screen a seller turns towards the customer: the code, the name, the
    description and the price, and nothing that belongs to the seller's side
    of the sale.
  -->
  <UModal
    v-if="record"
    v-model:open="presentOpen"
    :title="record.name || t('sell.presentFallbackTitle')"
    :transition="false"
    :ui="{ content: 'max-w-md' }"
  >
    <template #body>
      <div class="flex flex-col items-center gap-4">
        <!-- 400 rather than the card's 320: an invoice link is long, so the
             code is dense, and it is read from further away here. It still
             shrinks to fit a narrow screen. -->
        <PaymentQr
          :payload="qrPayload"
          :size="400"
        />

        <div class="w-full text-center">
          <p
            v-if="record.description"
            class="text-sm text-muted"
          >
            {{ record.description }}
          </p>
          <p class="mt-2 text-2xl font-semibold tabular-nums">
            {{ paidAmountLabel(record) }}
          </p>
        </div>

        <p class="text-center text-xs text-muted">
          {{ t('sell.scanToPay') }}
        </p>
      </div>
    </template>
  </UModal>
</template>
