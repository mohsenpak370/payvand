<script setup lang="ts">
import type { SellPageState } from '~/composables/useSellPage'

const props = defineProps<{
  sell: SellPageState
}>()

const { t } = useI18n()

const {
  CODE_SIZE,
  codePaperWidth,
  record,
  qrPayload,
  createdItems,
  shareLink,
  copiedCode,
  copiedLink,
  payloadBox,
  savedLabel,
  paidCode,
  presentOpen,
  codeOpen,
  statusColor,
  reset,
  copyLink,
  copyPayload,
  paidAmountLabel
} = props.sell
</script>

<template>
  <UCard>
    <template #header>
      <!-- A heading and its button are one column each: in one row on a phone
           the heading broke apart instead of the button moving down. -->
      <div class="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between sm:gap-3">
        <h2 class="text-base font-semibold whitespace-nowrap">
          {{ t('sell.showHeading') }}
        </h2>
        <UButton
          v-if="record"
          :label="t('sell.newPayment')"
          color="neutral"
          variant="soft"
          size="sm"
          @click="reset()"
        />
      </div>
    </template>

    <div
      v-if="qrPayload && record"
      class="flex flex-col items-center gap-4"
    >
      <!-- The code and the way to present it share one width. -->
      <div
        class="flex w-full flex-col items-center gap-4"
        :style="{ maxWidth: codePaperWidth }"
      >
        <PaymentQr
          :payload="qrPayload"
          :size="CODE_SIZE"
        />

        <!--
          The seller is the one showing the code, usually on this phone, so
          the way to hand it over is here rather than on the payment page the
          customer would have to open first.
        -->
        <UButton
          :label="t('sell.showToCustomer')"
          icon="i-lucide-maximize"
          block
          @click="presentOpen = true"
        />
      </div>

      <p class="text-center text-xs text-muted">
        {{ t('sell.scanHint') }}
      </p>

      <div class="flex flex-wrap items-center justify-center gap-2">
        <UButton
          :to="shareLink"
          target="_blank"
          rel="noopener"
          :label="t('sell.openPage')"
          icon="i-lucide-external-link"
        />
        <UButton
          :icon="copiedLink ? 'i-lucide-check' : 'i-lucide-link'"
          :label="copiedLink ? t('sell.linkCopied') : t('sell.copyLink')"
          color="neutral"
          variant="soft"
          @click="copyLink()"
        />
      </div>

      <div
        v-if="record.name || record.description"
        class="text-center"
      >
        <p
          v-if="record.name"
          class="text-base font-medium"
        >
          {{ record.name }}
        </p>
        <p
          v-if="record.description"
          class="mt-0.5 text-sm text-muted"
        >
          {{ record.description }}
        </p>
      </div>

      <!-- The receipt the customer sees, kept from the invoice itself: a
           status refresh replaces the record, not this. -->
      <div
        v-if="createdItems.length"
        class="w-full rounded-xl border border-default text-start"
      >
        <ul class="divide-y divide-default">
          <li
            v-for="(item, index) in createdItems"
            :key="index"
            class="flex items-baseline justify-between gap-3 px-3 py-1.5 text-sm"
          >
            <span class="min-w-0 break-words">{{ item.quantity }} × {{ item.name }}</span>
            <!-- Each line is priced in the invoice's own unit, which for a
                 shop in EURC is EURC, not the SOL the line's field is named
                 after. -->
            <span class="shrink-0 text-muted tabular-nums">
              {{ priceInCurrency(item.amount_lamports * item.quantity, record.options?.[0]?.currency ?? 'sol') }}
            </span>
          </li>
        </ul>
      </div>

      <div class="flex items-center gap-2">
        <UBadge
          :color="statusColor(record.status)"
          variant="subtle"
          size="lg"
        >
          {{ statusLabel(record.status) }}
        </UBadge>
        <span class="text-sm text-muted">
          {{ paidAmountLabel(record) }}
          <template v-if="record.memo"> · {{ record.memo }}</template>
        </span>
        <UBadge
          v-if="record.discount"
          color="success"
          variant="subtle"
        >
          {{ t('sell.percentOff', { percent: record.discount.percent_off }) }}
        </UBadge>
        <UBadge
          v-if="paidCode && savedLabel"
          color="success"
          variant="subtle"
        >
          {{ t('sell.savedBadge', { code: paidCode, amount: savedLabel }) }}
        </UBadge>
      </div>

      <!--
        Fallbacks and identifiers: needed when a phone will not scan, or when
        a payment has to be looked up. Folded so the code is what the card
        shows, which is what the seller actually holds up.
      -->
      <UButton
        v-if="!codeOpen"
        :label="t('sell.showCodeText')"
        icon="i-lucide-chevron-down"
        color="neutral"
        variant="ghost"
        size="xs"
        :aria-expanded="false"
        @click="codeOpen = true"
      />

      <template v-else>
        <dl class="w-full space-y-1 text-xs text-muted">
          <div class="flex justify-between gap-3">
            <dt>{{ t('sell.paymentId') }}</dt>
            <dd class="truncate font-mono">
              {{ record.payment_pda }}
            </dd>
          </div>
          <div
            v-if="record.settled_by"
            class="flex justify-between gap-3"
          >
            <dt>{{ t('sell.paidBy') }}</dt>
            <dd class="truncate font-mono">
              {{ record.settled_by }}
            </dd>
          </div>
        </dl>

        <!--
          The same text the pay page accepts pasted in, so a customer who
          cannot scan the QR can still pay. `break-all` matters: the payload
          is one unbroken JSON string, and without it the card would grow to
          the payload's width instead of wrapping inside the card.
        -->
        <div :ref="(el) => { payloadBox = el as HTMLElement | undefined }">
          <div class="mb-1.5 flex items-center justify-between gap-3">
            <span class="text-sm font-medium">{{ t('sell.paymentCode') }}</span>
            <UButton
              :icon="copiedCode ? 'i-lucide-check' : 'i-lucide-copy'"
              :label="copiedCode ? t('common.copied') : t('sell.copyCode')"
              color="neutral"
              variant="soft"
              size="xs"
              @click="copyPayload()"
            />
          </div>
          <code class="block w-full rounded-md border border-default bg-elevated/40 px-2.5 py-2 font-mono text-xs break-all whitespace-pre-wrap">
            {{ qrPayload }}
          </code>
          <p class="mt-1.5 text-xs text-muted">
            {{ t('sell.pasteHint') }}
          </p>
        </div>

        <UButton
          :label="t('sell.hideDetailsButton')"
          icon="i-lucide-chevron-up"
          color="neutral"
          variant="ghost"
          size="xs"
          :aria-expanded="true"
          @click="codeOpen = false"
        />
      </template>
    </div>

    <div
      v-else
      class="py-10 text-center text-sm text-muted"
    >
      {{ t('sell.emptyCode') }}
    </div>
  </UCard>
</template>
