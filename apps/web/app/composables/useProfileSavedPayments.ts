import type { Ref } from 'vue'
import type { PaymentQrPayload, SavedPayment } from './usePaymentApi'

/** A saved code with the invoice decoded and its shareable link ready. */
export interface SavedPaymentCard extends SavedPayment {
  code: PaymentQrPayload
  link: string
}

interface UseProfileSavedPaymentsOptions {
  shopsEnabled: Readonly<Ref<boolean>>
}

/**
 * Codes this account saved, read from the API. The code is the invoice, so the
 * card shows details parsed out of its payload rather than fetched separately.
 */
export function useProfileSavedPayments({
  shopsEnabled
}: UseProfileSavedPaymentsOptions) {
  const { t } = useI18n()
  const paymentApi = usePaymentApi()

  const savedPayments = ref<SavedPayment[]>([])
  const savedLoading = ref(false)
  const savedFailure = ref('')

  const savedCount = computed(() => savedPayments.value.length)

  const savedCards = computed<SavedPaymentCard[]>(() => savedPayments.value.flatMap((item) => {
    try {
      return [{ ...item, code: parseQrPayload(item.payload), link: paymentUrl(item.payload) }]
    } catch {
      // A code that no longer parses is not worth showing.
      return []
    }
  }))

  async function refreshSavedPayments() {
    savedFailure.value = ''
    savedLoading.value = true
    try {
      savedPayments.value = await paymentApi.listSavedPayments()
    } catch (error) {
      savedFailure.value = apiErrorMessage(error, t('profile.savedFailure'))
    } finally {
      savedLoading.value = false
    }
  }

  onMounted(() => {
    if (shopsEnabled.value) refreshSavedPayments()
  })

  return {
    savedCards,
    savedCount,
    savedLoading,
    savedFailure,
    refreshSavedPayments
  }
}
