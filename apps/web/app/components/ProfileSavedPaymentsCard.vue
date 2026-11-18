<script setup lang="ts">
import type { PaymentQrPayload } from '~/composables/usePaymentApi'
import type { SavedPaymentCard } from '~/composables/useProfileSavedPayments'

/**
 * Codes saved to this account, each shown from the invoice its QR payload
 * holds. useProfileSavedPayments loads and decodes them; the fold is local UI
 * state, so it stays with the card that owns the button.
 */
defineProps<{
  cards: SavedPaymentCard[]
  count: number
  loading: boolean
  failure: string
}>()

const emit = defineEmits<{ refresh: [] }>()

const savedOpen = ref(false)

/** What the buyer pays: a saved code may carry its own discount. */
function cardPrice(code: PaymentQrPayload) {
  return code.discount
    ? discountedLamports(code.amount_lamports, code.discount.percent_off)
    : code.amount_lamports
}
</script>

<template>
  <UCard class="min-w-0">
    <template #header>
      <div class="flex items-center justify-between gap-3">
        <div>
          <h2 class="text-base font-semibold">
            {{ $t('profile.savedTitle') }}
          </h2>
          <p class="text-sm text-muted">
            {{ $t('profile.savedDescription') }}
          </p>
        </div>
        <UButton
          icon="i-lucide-refresh-cw"
          :label="$t('profile.refresh')"
          color="neutral"
          variant="soft"
          size="sm"
          :loading="loading"
          @click="emit('refresh')"
        />
      </div>
    </template>

    <div class="flex flex-col gap-3">
      <UAlert
        v-if="failure"
        color="error"
        variant="subtle"
        icon="i-lucide-circle-alert"
        :title="$t('profile.savedErrorTitle')"
        :description="failure"
      />

      <p
        v-else-if="loading && !count"
        class="py-4 text-center text-sm text-muted"
      >
        {{ $t('profile.savedLoading') }}
      </p>

      <p
        v-else-if="!count"
        class="py-4 text-center text-sm text-muted"
      >
        {{ $t('profile.savedEmpty') }}
      </p>

      <UButton
        v-else-if="!savedOpen"
        :label="$t('profile.showSaved', { count: cards.length }, cards.length)"
        icon="i-lucide-chevron-down"
        color="neutral"
        variant="ghost"
        block
        @click="savedOpen = true"
      />

      <ul
        v-else
        class="grid gap-3 sm:grid-cols-2"
      >
        <li
          v-for="card in cards"
          :key="card.id"
          class="flex min-w-0 items-start gap-3 rounded-xl border border-default p-3"
        >
          <PaymentQr
            :payload="card.payload"
            :size="96"
            class="shrink-0"
          />
          <div class="flex min-w-0 flex-col gap-0.5">
            <p class="text-sm font-medium break-words">
              {{ card.code.name || $t('profile.paymentFallback') }}
            </p>
            <p class="text-sm font-semibold">
              {{ lamportsToSol(cardPrice(card.code)) }} SOL
              <span
                v-if="card.code.discount"
                class="ms-1 text-xs font-normal text-muted line-through"
              >{{ lamportsToSol(card.code.amount_lamports) }} SOL</span>
            </p>
            <p
              v-if="card.code.description || card.code.memo"
              class="text-xs text-muted break-words"
            >
              {{ card.code.description || card.code.memo }}
            </p>
            <p class="text-xs text-muted break-words">
              {{ networkLabel(card.code.cluster) }} · {{ formatDate(card.created_at_unix) }}
            </p>
            <UButton
              :to="card.link"
              target="_blank"
              rel="noopener"
              :label="$t('profile.openPayment')"
              icon="i-lucide-external-link"
              color="neutral"
              variant="soft"
              size="xs"
              class="mt-1 self-start"
            />
          </div>
        </li>
      </ul>

      <UButton
        v-if="savedOpen"
        :label="$t('profile.hide')"
        icon="i-lucide-chevron-up"
        color="neutral"
        variant="ghost"
        block
        @click="savedOpen = false"
      />
    </div>
  </UCard>
</template>
