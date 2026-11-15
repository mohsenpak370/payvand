import { address, getAddressDecoder, type Address } from '@vue-solana/nuxt/kit'
import {
  PAYMENT_RECEIPT_SPACE,
  PAYMENT_SPACE,
  TOKEN_ACCOUNT_SPACE,
  TOKEN_PAYMENT_SPACE,
  invoiceCosts,
  type InvoiceCosts
} from '~/utils/invoiceCost'
import type { SupportedCurrency } from './usePaymentApi'
import {
  accountDiscriminator,
  deriveAssociatedTokenAddress,
  deriveConfigPda
} from './usePaymentProgram'

/**
 * The fee policy the program reads at settlement time.
 *
 * The client quotes costs from this rather than from copied constants: a
 * policy update on chain (a different fee rate, a larger session float) then
 * shows up in the next quote instead of quoting yesterday's numbers.
 */
export interface FeeConfig {
  admin: Address
  /** Seller-side fee in basis points (200 = 2%). */
  feeBps: number
  fixedFeeLamports: number
  feeMode: 'seller' | 'buyer'
  /** Lamports `open_budget` parks on a session key. */
  sessionFeeFloat: number
}

/**
 * `CONFIG_SPACE` from the program: 8 (discriminator) + 32 (admin) + 2 (fee_bps)
 * + 8 (fixed_fee_lamports) + 1 (fee_mode) + 8 (session_fee_float) + 1 (bump) +
 * 1 (treasury_bump) + 1 (version).
 */
const CONFIG_SIZE = 62

/**
 * How long a read stays fresh. Long enough that a checkout does not re-read the
 * account per keystroke, short enough that an admin policy change reaches the
 * next quote within a minute.
 */
const CACHE_TTL_MS = 60_000

let cache: { readAt: number, config: FeeConfig } | null = null

/** Drops the cache, so the next quote reads the account again. */
export function clearFeeConfigCache(): void {
  cache = null
}

/** Reads the singleton fee policy, cached for `CACHE_TTL_MS`. */
export function useFeeConfig() {
  const { client } = useSolanaClient()

  async function load(options: { force?: boolean } = {}): Promise<FeeConfig> {
    if (!options.force && cache && Date.now() - cache.readAt < CACHE_TTL_MS) {
      return cache.config
    }

    const pda = await deriveConfigPda()
    const info = await client.rpc
      .getAccountInfo(pda, { commitment: 'confirmed', encoding: 'base64' })
      .send()
    if (!info.value || !Array.isArray(info.value.data)) {
      // Better to refuse to quote than to assume a zero fee: a wrong total is
      // what makes a transaction fail its pre-flight, or charge the wrong
      // amount.
      throw new Error(appT('errors.feeConfigUnavailable'))
    }

    const bytes = Uint8Array.from(atob(info.value.data[0]), char => char.charCodeAt(0))
    const expected = await accountDiscriminator('Config')
    const tagged = bytes.length >= CONFIG_SIZE && expected.every((byte, i) => bytes[i] === byte)
    if (!tagged) {
      throw new Error(appT('errors.feeConfigUnavailable'))
    }

    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
    const config: FeeConfig = {
      admin: getAddressDecoder().decode(bytes.subarray(8, 40)),
      feeBps: view.getUint16(40, true),
      fixedFeeLamports: Number(view.getBigUint64(42, true)),
      feeMode: bytes[50] === 1 ? 'buyer' : 'seller',
      sessionFeeFloat: Number(view.getBigUint64(51, true))
    }

    cache = { readAt: Date.now(), config }
    return config
  }

  return { load }
}

/**
 * Quotes one invoice from the on-chain policy and the cluster's rent, so every
 * number a screen shows comes from the same place the program reads. The
 * arithmetic is `invoiceCosts` in `app/utils/invoiceCost.ts`.
 */
export function useInvoiceCosts() {
  const { client } = useSolanaClient()
  const feeConfig = useFeeConfig()
  const rentCache = new Map<number, number>()

  async function rentFor(space: number): Promise<number> {
    const cached = rentCache.get(space)
    if (cached !== undefined) return cached
    const lamports = Number(
      await client.rpc.getMinimumBalanceForRentExemption(BigInt(space)).send()
    )
    rentCache.set(space, lamports)
    return lamports
  }

  /**
   * Costs for paying `amount` in `currency`. `mint` and `seller` are optional
   * and only used to tell whether a token payment would have to create the
   * seller's token account.
   */
  async function estimateInvoiceCosts(
    amount: number,
    currency: SupportedCurrency = 'sol',
    options: { mint?: string, seller?: Address } = {}
  ): Promise<InvoiceCosts> {
    const policy = await feeConfig.load()
    const isToken = currency !== 'sol'

    const invoiceRent = await rentFor(isToken ? TOKEN_PAYMENT_SPACE : PAYMENT_SPACE)
    const receiptRent = await rentFor(PAYMENT_RECEIPT_SPACE)

    // A seller prices in a token only once their account exists, so a code
    // without one was made elsewhere: this is what the checkout tells the buyer
    // to ask the seller for, never a lamport the buyer pays.
    let sellerTokenAccountRent = 0
    if (isToken && options.mint && options.seller) {
      const account = await deriveAssociatedTokenAddress(options.seller, address(options.mint))
      const info = await client.rpc
        .getAccountInfo(account, { commitment: 'confirmed', encoding: 'base64' })
        .send()
      if (!info.value) sellerTokenAccountRent = await rentFor(TOKEN_ACCOUNT_SPACE)
    }

    return invoiceCosts({
      policy,
      amount,
      currency,
      invoiceRent,
      receiptRent,
      sellerTokenAccountRent
    })
  }

  return { estimateInvoiceCosts }
}
