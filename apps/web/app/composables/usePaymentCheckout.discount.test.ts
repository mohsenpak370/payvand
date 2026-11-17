import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { discountedLamports } from './usePaymentApi'
import { usePaymentCheckout } from './usePaymentCheckout'
import {
  quoteFor,
  reference,
  seller,
  sendSilentTokenPayment,
  setupCheckout,
  teardownCheckout,
  tokenFloatTopUpInstructions
} from '../test-utils/usePaymentCheckout'
import { derivePaymentPda } from './usePaymentProgram'

beforeEach(setupCheckout)
afterEach(teardownCheckout)

/**
 * A code that carries a discount prices itself at load: `applyDiscount` quotes
 * costs and budget for the discounted price. `load()` used to run that same pair
 * again, so a discounted checkout read the fee config and the allowance twice
 * before the buyer saw a price. These count both reads.
 */
describe('the quotes at load for a code that carries a discount', () => {
  const amount = 250_000
  let estimateInvoiceCosts: ReturnType<typeof vi.fn>
  let loadBudget: ReturnType<typeof vi.fn>
  let validateDiscount: ReturnType<typeof vi.fn>

  beforeEach(() => {
    // `load()` leaves a record poll behind; a faked clock keeps it from firing.
    vi.useFakeTimers()
    estimateInvoiceCosts = vi.fn(async () => quoteFor(false))
    loadBudget = vi.fn(async () => null)
    validateDiscount = vi.fn(async () => ({
      code: 'SAVE10',
      percent_off: 10,
      discounted_lamports: discountedLamports(amount, 10),
      saved_lamports: amount - discountedLamports(amount, 10)
    }))
    vi.stubGlobal('apiErrorMessage', (_: unknown, fallback: string) => fallback)
    vi.stubGlobal('useInvoiceCosts', () => ({ estimateInvoiceCosts }))
    vi.stubGlobal('useBudget', () => ({
      loadBudget,
      loadTokenBudget: vi.fn(async () => null),
      sendSilentTokenPayment,
      tokenFloatTopUpInstructions
    }))
    vi.stubGlobal('unwrapQrText', (text: string) => text)
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  async function loadCode(discount?: { code: string, percent_off: number }) {
    const invoice = (await derivePaymentPda(seller, reference, amount)).toString()
    vi.stubGlobal('parseQrPayload', () => ({
      v: 1,
      id: invoice,
      seller,
      reference,
      amount_lamports: amount,
      memo: '',
      name: 'Discounted invoice',
      seller_user: 'seller-user-id',
      cluster: 'devnet',
      endpoint: 'https://api.devnet.solana.com',
      ...(discount ? { discount } : {})
    }))
    vi.stubGlobal('usePaymentApi', () => ({ getPayment: async () => null, validateDiscount }))
    const checkout = usePaymentCheckout()
    await checkout.load('payvand://discounted')
    return checkout
  }

  it('quotes costs and budget once for a discounted checkout', async () => {
    const checkout = await loadCode({ code: 'SAVE10', percent_off: 10 })

    expect(checkout.discount.value?.code).toBe('SAVE10')
    expect(estimateInvoiceCosts).toHaveBeenCalledTimes(1)
    expect(loadBudget).toHaveBeenCalledTimes(1)
  })

  it('quotes costs and budget once on a full-price load too', async () => {
    await loadCode()

    expect(estimateInvoiceCosts).toHaveBeenCalledTimes(1)
    expect(loadBudget).toHaveBeenCalledTimes(1)
  })

  it('still quotes the full price when the carried code fails validation', async () => {
    validateDiscount.mockRejectedValueOnce(new Error('expired'))
    const checkout = await loadCode({ code: 'EXPIRED', percent_off: 10 })

    expect(checkout.discount.value).toBeNull()
    expect(estimateInvoiceCosts).toHaveBeenCalledTimes(1)
    expect(loadBudget).toHaveBeenCalledTimes(1)
  })
})
