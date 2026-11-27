import type { ReusableLink, SupportedCurrency } from '~/composables/usePaymentApi'

export function useSellPage(shopId?: string) {
  const { cluster } = useRuntimeConfig().public
  const subscription = useSubscription()

  /**
   * Whether this page was opened for one shop. Its menu, its codes and its card
   * designs are that page's features; the plain payment page prices one typed
   * amount and shows nothing from the shop feature at all.
   */
  const shopScoped = computed(() => Boolean(shopId))

  const discountCodes = useSellDiscountCodes()

  /**
   * The shop feature switched on *and* paid for. The switch is only a wish — it
   * says the person wants shops in the app — while the subscription is what
   * activates them, so the shop-only sections wait for both.
   */
  const shopsActivated = ref(false)

  async function loadActivation(): Promise<void> {
    if (!shopScoped.value) return
    try {
      shopsActivated.value = (await subscription.status()).active
    } catch {
      // A read that fails proves nothing either way, so the sections stay as
      // they were rather than being taken away from a seller who has paid.
    }
  }

  // Shared by the form (source of truth) and the result (saved-label fallback).
  const priceCurrency = ref<SupportedCurrency>('sol')

  /**
   * The reusable code just made, if that is what was made. It replaces the
   * invoice on the result card: there is no single invoice to show, only the
   * offer every visitor will pay.
   */
  const reusable = ref<ReusableLink | null>(null)

  const paymentResult = useSellPaymentResult({
    discounts: discountCodes.discounts,
    priceCurrency
  })

  const invoiceForm = useSellInvoiceForm({
    cluster,
    shopId,
    selectedDiscount: discountCodes.selectedDiscount,
    allowDiscounts: discountCodes.allowDiscounts,
    priceCurrency,
    applyCreated: (created) => {
      reusable.value = null
      paymentResult.applyCreated(created)
      invoiceForm.clearFailure()
    },
    applyReusable: (link) => {
      paymentResult.reset()
      reusable.value = link
    }
  })

  return buildSellPage({
    cluster,
    shopScoped,
    shopsActivated,
    loadActivation,
    discountCodes,
    invoiceForm,
    paymentResult,
    reusable
  })
}

function buildSellPage(state: {
  cluster: string
  shopScoped: Ref<boolean>
  shopsActivated: Ref<boolean>
  loadActivation: () => Promise<void>
  discountCodes: ReturnType<typeof useSellDiscountCodes>
  invoiceForm: ReturnType<typeof useSellInvoiceForm>
  paymentResult: ReturnType<typeof useSellPaymentResult>
  reusable: Ref<ReusableLink | null>
}) {
  const { cluster, shopScoped, shopsActivated, loadActivation, discountCodes, invoiceForm, paymentResult, reusable } = state

  onMounted(() => {
    // The seller's codes and the shop's subscription are only read where they
    // are shown; the plain payment page asks for neither.
    if (shopScoped.value) {
      void discountCodes.loadDiscounts()
      void loadActivation()
    }
    void invoiceForm.loadShop()
  })

  function reset() {
    invoiceForm.clearFailure()
    reusable.value = null
    paymentResult.reset()
  }

  return {
    cluster,
    // `shopScoped` rides along with the form's own state.
    shopsActivated,
    reusable,
    ...discountCodes,
    ...invoiceForm,
    ...paymentResult,
    reset
  }
}

export type SellPageState = ReturnType<typeof useSellPage>
