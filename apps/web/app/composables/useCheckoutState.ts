import type { Ref } from 'vue'
import type {
  DiscountPreview,
  PaymentQrPayload,
  PaymentRecord,
  SupportedCurrency
} from './usePaymentApi'

/**
 * The invoice's mutable state, created once and shared by every checkout module.
 *
 * The `discount` ref lives here rather than inside the pricing module so the
 * amount computeds and the discount that changes them can both be built in one
 * pass: pricing reads the ref, and the discount module writes it, without either
 * module needing the other to exist first.
 */
export interface PaymentCheckoutState {
  payload: Ref<PaymentQrPayload | null>
  /** The seller's own code, verbatim, so the QR can re-encode exactly it. */
  code: Ref<string>
  record: Ref<PaymentRecord | null>
  loading: Ref<boolean>
  error: Ref<string>
  failure: Ref<string>
  busy: Ref<boolean>
  signature: Ref<string | null>
  selectedCurrency: Ref<SupportedCurrency>
  activePda: Ref<string | null>
  discount: Ref<DiscountPreview | null>
  discountInput: Ref<string>
  discountBusy: Ref<boolean>
  discountError: Ref<string>
}

export function useCheckoutState(): PaymentCheckoutState {
  return {
    payload: ref<PaymentQrPayload | null>(null),
    code: ref(''),
    record: ref<PaymentRecord | null>(null),
    loading: ref(true),
    error: ref(''),
    failure: ref(''),
    busy: ref(false),
    signature: ref<string | null>(null),
    selectedCurrency: ref<SupportedCurrency>('sol'),
    activePda: ref<string | null>(null),
    discount: ref<DiscountPreview | null>(null),
    discountInput: ref(''),
    discountBusy: ref(false),
    discountError: ref('')
  }
}
