import { address } from '@vue-solana/nuxt/kit'
import { vi } from 'vitest'
import { invoiceCosts, type InvoiceCosts } from '~/utils/invoiceCost'
import { usePaymentCheckout } from '~/composables/usePaymentCheckout'
import type { SponsoredSettleInput } from '~/composables/usePaymentApi'

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
/** What the wallet signs when a relayer pays the fee; it hands the input back. */
export const signTransactions = vi.fn(async (transactions: Uint8Array[]) => transactions)
/** The relayer's own key, named as fee payer when the buyer cannot pay it. */
export const sponsor = address('9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM')
export const getSponsor = vi.fn(async () => ({
  available: false,
  address: null as string | null,
  needs_lamports: 3_944_360,
  per_wallet_daily: 10
}))
export const sponsorSettle = vi.fn(async (_body: SponsoredSettleInput) => ({
  signature: 'SPONSORED_SIGNATURE'
}))
/** What the wallet holds, in lamports: a funded wallet by default. */
export const walletLamports = { value: 5_000_000 }
/** What the wallet holds in the invoice's token, in base units. */
export const walletTokens = { value: 5_000_000 }
/** Whether the buyer's token account exists, and whether the cluster answers. */
export const walletTokenAccount = { exists: true, unreachable: false }
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

/** A SOL invoice, priced the way the program prices one. */
export function quoteForSol(amount: number): InvoiceCosts {
  return invoiceCosts({
    policy: {
      feeBpsBelow: 200,
      feeBpsAbove: 50,
      solTierLamports: 100_000_000,
      tokenTierBaseUnits: 0,
      flatUsdcBaseUnits: 0,
      flatEurcBaseUnits: 0,
      usdcMint: mint,
      eurcMint: mint,
      feeMode: 'seller'
    },
    amount,
    currency: 'sol',
    invoiceRent: 1_000_000,
    receiptRent: 1_000_000
  })
}

/** A SOL invoice at `amount` lamports, with no code loaded. */
export function checkoutForSol(amount: number) {
  const checkout = usePaymentCheckout()
  checkout.payload.value = {
    v: 1,
    id: reference,
    seller,
    reference,
    amount_lamports: amount,
    options: [{ currency: 'sol', mint: '', decimals: 9, amount, payment_pda: reference }],
    memo: '',
    name: 'Coffee',
    cluster: 'devnet',
    endpoint: 'https://api.devnet.solana.com'
  }
  checkout.costs.value = quoteForSol(amount)
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
  signTransactions.mockClear()
  signTransactions.mockImplementation(async (transactions: Uint8Array[]) => transactions)
  getSponsor.mockClear()
  getSponsor.mockImplementation(async () => ({
    available: false,
    address: null,
    needs_lamports: 3_944_360,
    per_wallet_daily: 10
  }))
  sponsorSettle.mockClear()
  sponsorSettle.mockImplementation(async () => ({ signature: 'SPONSORED_SIGNATURE' }))
  walletLamports.value = 5_000_000
  walletTokens.value = 5_000_000
  walletTokenAccount.exists = true
  walletTokenAccount.unreachable = false
  wallet.canSign = true

  vi.stubGlobal('ref', ref)
  vi.stubGlobal('computed', computed)
  vi.stubGlobal('watch', watch)
  vi.stubGlobal('onBeforeUnmount', () => {})
  vi.stubGlobal('appT', (key: string) => key)
  vi.stubGlobal('buildInstruction', buildInstructionMock)
  vi.stubGlobal('compileWireTransaction', async () => ({}))
  vi.stubGlobal('compileTransactionForSigning', async () => ({}))
  vi.stubGlobal('signableWireTransaction', () => new Uint8Array())
  vi.stubGlobal('base64WireTransaction', () => 'BASE64_WIRE')
  vi.stubGlobal('usePaymentApi', () => ({
    getPayment: async () => null,
    validateDiscount: async () => null,
    getSponsor,
    sponsorSettle
  }))
  // The balance read is the one chain call the sponsored branch makes before it
  // builds anything, so it is what decides between the two paths.
  vi.stubGlobal('useSolanaClient', () => ({
    client: {
      rpc: {
        getBalance: () => ({ send: async () => ({ value: walletLamports.value }) }),
        getAccountInfo: () => ({
          send: async () => {
            if (walletTokenAccount.unreachable) throw new Error('fetch failed')
            return { value: walletTokenAccount.exists ? { data: ['', 'base64'] } : null }
          }
        }),
        getTokenAccountBalance: () => ({
          send: async () => ({ value: { amount: String(walletTokens.value) } })
        })
      }
    }
  }))
  // The factory reads `wallet` when the composable is set up, so a test can
  // flip the signer before constructing its checkout.
  vi.stubGlobal('useSolanaWallet', () => ({
    publicKey: ref(payer),
    connected: ref(true),
    canSignTransaction: ref(wallet.canSign)
  }))
  vi.stubGlobal('useSolanaSignAndSendTransaction', () => ({ execute }))
  vi.stubGlobal('useSolanaSignTransactions', () => ({ execute: signTransactions }))
  vi.stubGlobal('useBudget', () => ({ sendSilentTokenPayment, tokenFloatTopUpInstructions }))
  vi.stubGlobal('useInvoiceCosts', () => ({ estimateInvoiceCosts: async () => null }))
}

export function teardownCheckout() {
  vi.unstubAllGlobals()
}
