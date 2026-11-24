import type { SupportedCurrency } from '~/composables/usePaymentApi'

export function useSellPage(shopId?: string) {
  const { cluster } = useRuntimeConfig().public
  const { shopsEnabled } = useShopPreference()
  const subscription = useSubscription()

  const discountCodes = useSellDiscountCodes()

  /**
   * The shop feature switched on *and* paid for. The switch is only a wish — it
   * says the person wants shops in the app — while the subscription is what
   * activates them, so the shop-only sections wait for both.
   */
  const shopsActivated = ref(false)

  async function loadActivation(): Promise<void> {
    if (!shopsEnabled.value) return
    try {
      shopsActivated.value = (await subscription.status()).active
    } catch {
      // A read that fails proves nothing either way, so the sections stay as
      // they were rather than being taken away from a seller who has paid.
    }
  }

  // Shared by the form (source of truth) and the result (saved-label fallback).
  const priceCurrency = ref<SupportedCurrency>('sol')

  const paymentResult = useSellPaymentResult({
    discounts: discountCodes.discounts,
    priceCurrency
  })

  const invoiceForm = useSellInvoiceForm({
    cluster,
    shopsEnabled,
    shopId,
    selectedDiscount: discountCodes.selectedDiscount,
    allowDiscounts: discountCodes.allowDiscounts,
    priceCurrency,
    applyCreated: (created) => {
      paymentResult.applyCreated(created)
      invoiceForm.clearFailure()
    }
  })

  return buildSellPage({
    cluster,
    shopsEnabled,
    shopsActivated,
    loadActivation,
    discountCodes,
    invoiceForm,
    paymentResult
  })
}

function buildSellPage(state: {
  cluster: string
  shopsEnabled: Ref<boolean>
  shopsActivated: Ref<boolean>
  loadActivation: () => Promise<void>
  discountCodes: ReturnType<typeof useSellDiscountCodes>
  invoiceForm: ReturnType<typeof useSellInvoiceForm>
  paymentResult: ReturnType<typeof useSellPaymentResult>
}) {
  const { cluster, shopsEnabled, shopsActivated, loadActivation, discountCodes, invoiceForm, paymentResult } = state

  onMounted(() => {
    void discountCodes.loadDiscounts()
    void invoiceForm.loadShop()
    void loadActivation()
  })

  function reset() {
    invoiceForm.clearFailure()
    paymentResult.reset()
  }

  return {
    cluster,
    shopsEnabled,
    shopsActivated,
    ...discountCodes,
    ...invoiceForm,
    ...paymentResult,
    reset
  }
}

export type SellPageState = ReturnType<typeof useSellPage>
