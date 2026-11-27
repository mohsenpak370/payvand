<script setup lang="ts">
import type { SellPageState } from '~/composables/useSellPage'
import { cardDesignSwatch } from '~/utils/cardDesigns'

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
  copiedCode,
  copiedLink,
  payloadBox,
  savedLabel,
  paidCode,
  presentOpen,
  design,
  designOptions,
  designOpen,
  openPaymentPage,
  codeOpen,
  statusColor,
  reset,
  copyLink,
  copyPayload,
  paidAmountLabel,
  reusable
} = props.sell

// A reusable code is shown as the link it is: the same code, pointing at the
// offer rather than at one invoice, so every visitor gets an invoice of their
// own instead of finding the first payer's already settled.
const origin = useRequestURL().origin
const reusableUrl = computed(
  () => (reusable.value ? `${origin}${reusable.value.path}` : '')
)
const reusablePrice = computed(() => {
  const link = reusable.value
  if (!link) return ''
  return link.currency === 'sol'
    ? `${lamportsToSol(link.amount)} SOL`
    : `${formatTokenAmount(link.amount, 6)} ${currencySymbol(link.currency)}`
})
const reusableCopied = ref(false)
let reusableCopiedTimer: ReturnType<typeof setTimeout> | undefined

async function copyReusable() {
  try {
    await navigator.clipboard.writeText(reusableUrl.value)
  } catch {
    return
  }
  reusableCopied.value = true
  clearTimeout(reusableCopiedTimer)
  reusableCopiedTimer = setTimeout(() => {
    reusableCopied.value = false
  }, 2_000)
}

onBeforeUnmount(() => clearTimeout(reusableCopiedTimer))
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
          v-if="record || reusable"
          :label="t('sell.newPayment')"
          color="neutral"
          variant="soft"
          size="sm"
          @click="reset()"
        />
      </div>
    </template>

    <!-- A reusable code, not an invoice: the price it charges and the way to
         hand the code over, with no single payment to watch. -->
    <div
      v-if="reusable"
      class="flex flex-col items-center gap-4"
    >
      <div
        class="flex w-full flex-col items-center gap-3"
        :style="{ maxWidth: codePaperWidth }"
      >
        <PaymentQr
          :url="reusable.path"
          :size="CODE_SIZE"
        />
        <UButton
          :icon="reusableCopied ? 'i-lucide-check' : 'i-lucide-link'"
          :label="reusableCopied ? t('sell.reusableCopied') : t('sell.reusableCopy')"
          block
          color="neutral"
          variant="soft"
          @click="copyReusable()"
        />
      </div>

      <div
        v-if="reusable.name || reusable.description"
        class="text-center"
      >
        <p
          v-if="reusable.name"
          class="text-base font-medium"
        >
          {{ reusable.name }}
        </p>
        <p
          v-if="reusable.description"
          class="mt-0.5 text-sm text-muted"
        >
          {{ reusable.description }}
        </p>
      </div>

      <p class="text-center text-sm font-medium tabular-nums">
        {{ reusablePrice }}
      </p>
      <p class="text-center text-sm text-muted">
        {{ t('sell.reusableHelp', { price: reusablePrice }) }}
      </p>
      <p class="text-center text-xs text-muted">
        {{ t('sell.reusableNote') }}
      </p>
    </div>

    <div
      v-else-if="qrPayload && record"
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
        <!--
          The page opens from a sheet that picks its look first: the design is
          the one thing about a made code that can still be chosen, and choosing
          it is part of opening the page rather than part of making it.
        -->
        <UButton
          :label="t('sell.openPage')"
          icon="i-lucide-external-link"
          @click="designOpen = true"
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

    <!--
      Five looks, each shown as the card itself rather than as a colour chip:
      the wash and the pattern are what tells them apart, and a preview that
      shows both is the only honest way to choose.
    -->
    <UModal
      v-model:open="designOpen"
      :title="t('sell.designLabel')"
      :description="t('sell.designHelp')"
    >
      <template #body>
        <div
          class="grid gap-3 sm:grid-cols-2"
          role="radiogroup"
          :aria-label="t('sell.designLabel')"
        >
          <button
            v-for="option in designOptions"
            :key="option.value"
            type="button"
            role="radio"
            :aria-checked="design === option.value"
            class="flex flex-col gap-2 rounded-xl border p-2 text-start transition-colors"
            :class="design === option.value
              ? 'border-primary ring-2 ring-primary/30'
              : 'border-default hover:border-primary/40'"
            @click="design = option.value"
          >
            <span
              class="block h-24 w-full overflow-hidden rounded-lg"
              :class="cardDesignSwatch(option.value)"
              aria-hidden="true"
            >
              <span class="flex h-full flex-col justify-between p-3">
                <span class="flex items-center justify-between gap-2">
                  <span class="text-[11px] font-semibold">Payvand</span>
                  <span class="rounded-full bg-white/30 px-1.5 py-0.5 text-[10px] font-medium text-default">4.50</span>
                </span>
                <span class="flex items-end justify-between gap-2">
                  <span class="min-w-0 truncate text-[11px] text-muted">{{ t('home.hero.card.memo') }}</span>
                  <span class="rounded-md bg-white/30 px-1.5 py-0.5 text-[10px] font-medium">Pay</span>
                </span>
              </span>
            </span>
            <span class="flex items-center justify-between gap-2 px-1 pb-1 text-xs">
              <span :class="design === option.value ? 'text-highlighted' : 'text-muted'">
                {{ option.label }}
              </span>
              <UIcon
                v-if="design === option.value"
                name="i-lucide-check"
                class="text-primary size-4"
              />
            </span>
          </button>
        </div>
      </template>

      <template #footer>
        <div class="flex w-full items-center justify-end gap-2">
          <UButton
            :label="t('common.cancel')"
            color="neutral"
            variant="ghost"
            @click="designOpen = false"
          />
          <UButton
            :label="t('sell.openPage')"
            icon="i-lucide-external-link"
            @click="openPaymentPage()"
          />
        </div>
      </template>
    </UModal>
  </UCard>
</template>
