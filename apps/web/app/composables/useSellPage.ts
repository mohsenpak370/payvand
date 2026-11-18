import type { SupportedCurrency } from '~/composables/usePaymentApi'

export function useSellPage() {
  const { cluster } = useRuntimeConfig().public
  const { shopsEnabled } = useShopPreference()

  const discountCodes = useSellDiscountCodes()

  // Shared by the form (source of truth) and the result (saved-label fallback).
  const priceCurrency = ref<SupportedCurrency>('sol')

  const paymentResult = useSellPaymentResult({
    discounts: discountCodes.discounts,
    priceCurrency
  })

  const invoiceForm = useSellInvoiceForm({
    cluster,
    shopsEnabled,
    selectedDiscount: discountCodes.selectedDiscount,
    priceCurrency,
    applyCreated: (created) => {
      paymentResult.applyCreated(created)
      invoiceForm.clearFailure()
    }
  })

  return buildSellPage({
    cluster,
    shopsEnabled,
    discountCodes,
    invoiceForm,
    paymentResult
  })
}

function buildSellPage(state: {
  cluster: string
  shopsEnabled: Ref<boolean>
  discountCodes: ReturnType<typeof useSellDiscountCodes>
  invoiceForm: ReturnType<typeof useSellInvoiceForm>
  paymentResult: ReturnType<typeof useSellPaymentResult>
}) {
  const { cluster, shopsEnabled, discountCodes, invoiceForm, paymentResult } = state

  onMounted(() => {
    void discountCodes.loadDiscounts()
    void invoiceForm.loadShop()
  })

  function reset() {
    invoiceForm.clearFailure()
    paymentResult.reset()
  }

  return {
    cluster,
    shopsEnabled,
    ...discountCodes,
    ...invoiceForm,
    ...paymentResult,
    reset
  }
}

export type SellPageState = ReturnType<typeof useSellPage>
