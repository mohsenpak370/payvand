import { getAddressDecoder, type Address } from '@vue-solana/nuxt/kit'
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

/** `PAYMENT_SPACE`: the temporary account an invoice is created in. */
const PAYMENT_SPACE = 230
/** `TOKEN_PAYMENT_SPACE`: its token counterpart. */
const TOKEN_PAYMENT_SPACE = 262
/** `PAYMENT_RECEIPT_SPACE`: the permanent record of a settled invoice. */
const PAYMENT_RECEIPT_SPACE = 301
/** An SPL token account (`getMinimumBalanceForRentExemption(165)`). */
const TOKEN_ACCOUNT_SPACE = 165

/**
 * One signature plus a margin: what the wallet charges to send either way. The
 * cluster fee is 5,000 lamports per signature; the rest covers a priority fee.
 */
const NETWORK_FEE_LAMPORTS = 10_000

/** What one invoice costs the seller, the payer and the protocol. */
export interface InvoiceCosts {
  currency: SupportedCurrency
  /** The price itself, in the currency's base units. */
  amount: number
  /** Protocol fee, in the same base units. */
  fee: number
  feeBps: number
  /** What the seller keeps, after a seller-paid fee. */
  sellerReceives: number
  /** What the buyer is charged on chain: the price, plus the fee only in BuyerPays. */
  payerPays: number
  /** Lamports: the invoice account's rent, returned to the seller when it closes. */
  invoiceRent: number
  /** Lamports: the permanent receipt, funded and refunded by the treasury. */
  receiptRent: number
  /** Lamports the payer adds on top of the price (network fee, token account). */
  payerExtras: number
  /** Lamports: the network fee estimate. */
  networkFee: number
  /** Lamports: the seller's token account rent, only when it has to be made. */
  sellerTokenAccountRent: number
  /** The smallest amount the program accepts under this policy, in base units. */
  minPayable: number
}

/**
 * Quotes one invoice from the on-chain policy and the cluster's rent, so every
 * number a screen shows comes from the same place the program reads. Nothing
 * here is a copied constant except the network fee estimate.
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

  /** The program's fee: proportional + fixed, capped at `amount - 1`. */
  function feeOn(config: FeeConfig, amount: number): number {
    if (amount <= 0) return 0
    const proportional = Math.floor((amount * config.feeBps) / 10_000)
    return Math.min(proportional + config.fixedFeeLamports, amount - 1)
  }

  /**
   * The smallest amount where `amount > rent + fee`: the program's
   * `InvoiceTooSmall` guard, solved rather than guessed. The closed form gets
   * within a lamport or two (floor and the cap move it), so it is corrected
   * locally; comparing against `rent + feeOn` keeps the fixed fee counted once,
   * which is what keeps that correction to a step or two.
   */
  function minimumPayable(config: FeeConfig, rent: number): number {
    if (config.feeBps >= 10_000) return Number.POSITIVE_INFINITY
    const base = rent + config.fixedFeeLamports
    let candidate = Math.max(
      1,
      Math.ceil(((base - 1) * 10_000) / (10_000 - config.feeBps))
    )
    while (candidate > 1 && candidate - 1 > rent + feeOn(config, candidate - 1)) {
      candidate--
    }
    while (candidate <= rent + feeOn(config, candidate)) {
      candidate++
    }
    return candidate
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
    const config = await feeConfig.load()
    const isToken = currency !== 'sol'

    const invoiceRent = await rentFor(isToken ? TOKEN_PAYMENT_SPACE : PAYMENT_SPACE)
    const receiptRent = await rentFor(PAYMENT_RECEIPT_SPACE)

    // A fresh seller has no token account, and the settle transaction creates
    // it with the payer's lamports, so that rent is the payer's cost.
    let sellerTokenAccountRent = 0
    if (isToken && options.mint && options.seller) {
      const account = await deriveAssociatedTokenAddress(options.seller, options.mint as Address)
      const info = await client.rpc
        .getAccountInfo(account, { commitment: 'confirmed', encoding: 'base64' })
        .send()
      if (!info.value) sellerTokenAccountRent = await rentFor(TOKEN_ACCOUNT_SPACE)
    }

    const fee = feeOn(config, amount)
    const sellerPays = config.feeMode === 'seller'

    return {
      currency,
      amount,
      fee,
      feeBps: config.feeBps,
      sellerReceives: sellerPays ? amount - fee : amount,
      payerPays: sellerPays ? amount : amount + fee,
      invoiceRent,
      receiptRent,
      networkFee: NETWORK_FEE_LAMPORTS,
      sellerTokenAccountRent,
      payerExtras: NETWORK_FEE_LAMPORTS + sellerTokenAccountRent,
      minPayable: minimumPayable(config, invoiceRent)
    }
  }

  return { estimateInvoiceCosts }
}
