<script setup lang="ts">
import type { SavedPayment } from '~/composables/usePaymentApi'

// The payment links this shop keeps. An invoice is never stored by the API —
// the code *is* the invoice — so listing one here is reading the code back and
// offering the same permanent link the seller handed out.
const props = defineProps<{ shopId: string }>()

const { t } = useI18n()
const api = usePaymentApi()

interface KeptCode {
  id: string
  label: string
  amount: string
  link: string
}

const codes = ref<KeptCode[]>([])
const loading = ref(true)
const failure = ref('')

/** A saved code as a row: what it is for, what it charges, where it opens. */
function describe(saved: SavedPayment): KeptCode | null {
  try {
    const payload = parseQrPayload(saved.payload)
    const option = paymentOptions(payload)[0]
    if (!option) return null
    return {
      id: saved.id,
      label: payload.name || payload.memo || t('shopCodes.fallbackLabel'),
      amount: option.currency === 'sol'
        ? `${lamportsToSol(option.amount)} SOL`
        : `${formatTokenAmount(option.amount, option.decimals)} ${currencySymbol(option.currency)}`,
      link: paymentLinkPath(saved.payload)
    }
  } catch {
    // A code that cannot be read is not a link anyone can use.
    return null
  }
}

async function load() {
  loading.value = true
  failure.value = ''
  try {
    const saved = await api.listSavedPayments()
    codes.value = saved
      .filter(item => item.shop_id === props.shopId)
      .map(describe)
      .filter((code): code is KeptCode => code !== null)
  } catch (error) {
    failure.value = apiErrorMessage(error, t('shopCodes.loadError'))
  } finally {
    loading.value = false
  }
}

onMounted(load)
</script>

<template>
  <UCard>
    <template #header>
      <div class="flex flex-wrap items-center justify-between gap-2">
        <h2 class="text-base font-semibold">
          {{ $t('shopCodes.title') }}
        </h2>
        <p class="text-xs text-muted">
          {{ $t('shopCodes.subtitle') }}
        </p>
      </div>
    </template>

    <p
      v-if="loading"
      class="text-sm text-muted"
    >
      {{ $t('shopCodes.loading') }}
    </p>

    <UAlert
      v-else-if="failure"
      color="error"
      variant="subtle"
      icon="i-lucide-circle-alert"
      :title="$t('shopCodes.loadError')"
      :description="failure"
    />

    <p
      v-else-if="!codes.length"
      class="text-sm text-muted"
    >
      {{ $t('shopCodes.empty') }}
    </p>

    <ul
      v-else
      class="divide-y divide-default"
    >
      <li
        v-for="code in codes"
        :key="code.id"
        class="flex items-center justify-between gap-3 py-2 first:pt-0 last:pb-0"
      >
        <div class="min-w-0">
          <p class="truncate text-sm font-medium">
            {{ code.label }}
          </p>
          <p class="text-xs text-muted tabular-nums">
            {{ code.amount }}
          </p>
        </div>
        <UButton
          :to="code.link"
          :label="$t('shopCodes.open')"
          icon="i-lucide-external-link"
          size="xs"
          color="neutral"
          variant="soft"
        />
      </li>
    </ul>
  </UCard>
</template>
