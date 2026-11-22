import type { DiscountCode } from '~/composables/usePaymentApi'

export function useSellDiscountCodes() {
  const { t } = useI18n()
  const api = usePaymentApi()

  const discounts = ref<DiscountCode[]>([])
  const newCode = ref('')
  const newPercent = ref('10')
  const codeBusy = ref(false)
  const codeFailure = ref('')
  const discountsOpen = ref(false)
  // Whether this payment lets the buyer apply one of the seller's codes. Off
  // by default: a seller who attached a code has already decided the price.
  const allowDiscounts = ref(false)

  // A code carried by the invoice itself: written into its QR, priced here from
  // the seller's own list, and applied by the buyer's screen without typing.
  // `NONE` rather than '': the select control reserves the empty string for its
  // placeholder and refuses it as an option value.
  const NONE = 'none'
  const discountCode = ref(NONE)

  const selectedDiscount = computed(
    () => discounts.value.find(code => code.code === discountCode.value) ?? null
  )

  const discountOptions = computed(() => [
    { label: t('sell.discountNone'), value: NONE },
    ...discounts.value.map(code => ({
      label: t('sell.discountOption', { code: code.code, percent: code.percent_off }),
      value: code.code
    }))
  ])

  /** "1 code" / "2 codes", in whatever forms the locale defines. */
  function codeCount(count: number) {
    return t('sell.codesCount', { count }, count)
  }

  const canAddCode = computed(() =>
    !codeBusy.value
    && newCode.value.trim().length >= 3
    && Number(newPercent.value) >= 1
    && Number(newPercent.value) <= 99
  )

  async function loadDiscounts() {
    try {
      discounts.value = await api.listDiscounts()
    } catch {
      // Signed out elsewhere — the middleware already guards this page.
    }
  }

  async function addDiscount() {
    codeFailure.value = ''
    codeBusy.value = true
    try {
      const created = await api.createDiscount({
        code: newCode.value,
        percent_off: Number(newPercent.value)
      })
      discounts.value = [created, ...discounts.value]
      newCode.value = ''
    } catch (error) {
      codeFailure.value = apiErrorMessage(error, t('sell.addDiscountFailure'))
    } finally {
      codeBusy.value = false
    }
  }

  async function removeDiscount(discount: DiscountCode) {
    codeFailure.value = ''
    try {
      await api.deleteDiscount(discount.id)
      discounts.value = discounts.value.filter(code => code.id !== discount.id)
    } catch (error) {
      codeFailure.value = apiErrorMessage(error, t('sell.deleteDiscountFailure'))
    }
  }

  return {
    discounts,
    newCode,
    newPercent,
    codeBusy,
    codeFailure,
    discountsOpen,
    allowDiscounts,
    discountCode,
    selectedDiscount,
    discountOptions,
    canAddCode,
    codeCount,
    loadDiscounts,
    addDiscount,
    removeDiscount
  }
}
