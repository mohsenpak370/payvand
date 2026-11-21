import { address } from '@vue-solana/nuxt/kit'
import { vi } from 'vitest'
import { invoiceCosts, type InvoiceCosts } from '~/utils/invoiceCost'
import { usePaymentCheckout } from '~/composables/usePaymentCheckout'

/**
 * Shared scaffolding for the `usePaymentCheckout` suites. The composable leans
 * on Nuxt's auto-imports, so each suite installs minimal versions of them as
 * globals. Each test file gets its own copy of these mocks (Vitest isolates
 * modules per file), and `setupCheckout` restores their state before every test.
 */

// Just enough reactivity for tests that only read: `vue` is not a direct
// dependency of this package, and nothing here needs effects or caching.
const ref = <T>(value: T) => ({ value })
const computed = <T>(compute: () => T) => ({
  get value(): T {
    return compute()
  }
})
const watch = (..._args: unknown[]) => {}

export const seller = address('6dQgFZDTkhDMTZv9ftZe4ufU9jG5fvJEw6zpw6WCKjPz')
export const payer = address('3rpBzUBbZPXFKhGywCRduD3hrREDjvDtaw2Mos2yUUPx')
export const mint = address('4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU')
export const reference = address('A8yxpWod2xrPd6YbXnaKWCTkk2WduzQKdKPUANZFQpft')

export const execute = vi.fn(async () => 'SIGNATURE')
export const sendSilentTokenPayment = vi.fn(async () => 'SILENT_SIGNATURE')
/** The transfers a spent float needs; most payments here need none. */
export const tokenFloatTopUpInstructions = vi.fn(async () => [] as unknown[])
// Named with a `Mock` suffix so it cannot be confused with the auto-import of
// the same name from `usePaymentProgram`.
export const buildInstructionMock = vi.fn(() => ({
  programAddress: reference,
  accounts: [],
  data: new Uint8Array()
}))

/** What the wallet reports about its own signer, per test. */
export const wallet = { canSign: true }

/** A 1.5 USDC invoice, priced the way the program prices one. */
export function quoteFor(treasuryTokenAccountMissing: boolean): InvoiceCosts {
  return invoiceCosts({
    policy: {
      feeBpsBelow: 200,
      feeBpsAbove: 200,
      solTierLamports: 0,
      tokenTierBaseUnits: 0,
      flatUsdcBaseUnits: 0,
      flatEurcBaseUnits: 0,
      usdcMint: mint,
      eurcMint: mint,
      feeMode: 'seller'
    },
    mint,
    amount: 1_500_000,
    currency: 'usdc',
    invoiceRent: 1_000_000,
    receiptRent: 1_000_000,
    treasuryTokenAccountMissing
  })
}

/** An invoice with no code loaded, priced by `quoteFor`. */
export function checkoutFor(quote: InvoiceCosts) {
  const checkout = usePaymentCheckout()
  checkout.payload.value = {
    v: 1,
    id: reference,
    seller,
    reference,
    amount_lamports: 0,
    options: [{ currency: 'usdc', mint, decimals: 6, amount: 1_500_000, payment_pda: reference }],
    memo: '',
    name: 'Treasury preflight',
    cluster: 'devnet',
    endpoint: 'https://api.devnet.solana.com'
  }
  checkout.costs.value = quote
  return checkout
}

/** The same invoice with a USDC allowance this device can sign for. */
export function withAllowance(checkout: ReturnType<typeof usePaymentCheckout>) {
  checkout.tokenBudgetState.value = {
    pda: reference,
    buyer: payer,
    sessionKey: payer,
    mint,
    monthlyLimit: 5_000_000,
    spent: 0,
    monthIndex: 1,
    lamports: 2_000_000,
    amount: 5_000_000
  }
  checkout.sessionReady.value = true
  checkout.record.value = {
    id: reference,
    status: 'pending',
    payment_pda: reference,
    seller,
    reference,
    amount_lamports: 0,
    memo: '',
    name: '',
    description: '',
    cluster: 'devnet',
    endpoint: 'https://api.devnet.solana.com',
    seller_user: null,
    settled_by: null,
    created_at_unix: 0,
    settled_at_unix: 0
  }
  return checkout
}

/** Installs the globals the composable expects, with every mock reset. */
export function setupCheckout() {
  execute.mockClear()
  buildInstructionMock.mockClear()
  sendSilentTokenPayment.mockClear()
  tokenFloatTopUpInstructions.mockClear()
  tokenFloatTopUpInstructions.mockImplementation(async () => [])
  wallet.canSign = true

  vi.stubGlobal('ref', ref)
  vi.stubGlobal('computed', computed)
  vi.stubGlobal('watch', watch)
  vi.stubGlobal('onBeforeUnmount', () => {})
  vi.stubGlobal('appT', (key: string) => key)
  vi.stubGlobal('buildInstruction', buildInstructionMock)
  vi.stubGlobal('compileWireTransaction', async () => ({}))
  vi.stubGlobal('usePaymentApi', () => ({
    getPayment: async () => null,
    validateDiscount: async () => null
  }))
  vi.stubGlobal('useSolanaClient', () => ({ client: { rpc: {} } }))
  // The factory reads `wallet` when the composable is set up, so a test can
  // flip the signer before constructing its checkout.
  vi.stubGlobal('useSolanaWallet', () => ({
    publicKey: ref(payer),
    connected: ref(true),
    canSignTransaction: ref(wallet.canSign)
  }))
  vi.stubGlobal('useSolanaSignAndSendTransaction', () => ({ execute }))
  vi.stubGlobal('useBudget', () => ({ sendSilentTokenPayment, tokenFloatTopUpInstructions }))
  vi.stubGlobal('useInvoiceCosts', () => ({ estimateInvoiceCosts: async () => null }))
}

export function teardownCheckout() {
  vi.unstubAllGlobals()
}
