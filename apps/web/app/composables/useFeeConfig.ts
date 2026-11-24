import { address, getAddressDecoder, type Address } from '@vue-solana/nuxt/kit'
import {
  PAYMENT_RECEIPT_SPACE,
  PAYMENT_SPACE,
  TOKEN_ACCOUNT_SPACE,
  TOKEN_PAYMENT_SPACE,
  invoiceCosts,
  type FeePolicy,
  type InvoiceCosts
} from '~/utils/invoiceCost'
import type { SupportedCurrency } from './usePaymentApi'
import {
  accountDiscriminator,
  deriveAssociatedTokenAddress,
  deriveConfigPda,
  deriveTreasuryTokenAccount
} from './usePaymentProgram'
import { clearRentCache, rentMinimum } from './rents'

/**
 * The fee policy the program reads at settlement time.
 *
 * The client quotes costs from this rather than from copied constants: a
 * policy update on chain (a different fee rate, a larger session float) then
 * shows up in the next quote instead of quoting yesterday's numbers.
 */
export interface FeeConfig extends FeePolicy {
  admin: Address
  /** Lamports `open_budget` parks on a session key. */
  sessionFeeFloat: number
}

/**
 * `CONFIG_SPACE` from the program: 8 (discriminator) + 32 (admin) + 124 (the
 * `FeePolicy`: two rates, their two boundaries, the two per-currency flats, the
 * two mints, and the network fee in lamports and in each token) + 1 (fee_mode)
 * + 8 (session_fee_float) + 8 (flats_updated_at) + 1 (bump) + 1
 * (treasury_bump) + 1 (version).
 *
 * The flats and the fee's token equivalents are read here at the SOL price the
 * last `refresh_fee_price` wrote, which is why a quote is only as stale as the
 * last refresh and not as stale as the last policy change.
 */
const CONFIG_SIZE = 184

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

/**
 * Whether an account exists, and when that answer was read.
 *
 * The two existence checks a token quote makes — the seller's account for the
 * price's mint, and the treasury's own account for it — answer the same question
 * on every quote, and a currency toggle asks them again within a click. They are
 * held for the same minute as the fee policy, so a toggle costs rent reads at
 * worst rather than two account reads on top of them.
 *
 * A missing account is held for that minute too, which is the trade the policy
 * cache already makes: the treasury's account is opened by the deployer and the
 * seller's by the seller, and both show up on the next quote within the minute.
 */
const accountCache = new Map<string, { readAt: number, exists: boolean }>()

/** Drops the cached existence reads, so the next quote asks the cluster again. */
export function clearInvoiceAccountCache(): void {
  accountCache.clear()
}

/** Drops the cached rents, so the next quote asks the cluster again. */
export function clearInvoiceRentCache(): void {
  clearRentCache()
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
    const addressAt = (offset: number) => getAddressDecoder().decode(bytes.subarray(offset, offset + 32))
    const config: FeeConfig = {
      admin: addressAt(8),
      feeBpsBelow: view.getUint16(40, true),
      feeBpsAbove: view.getUint16(42, true),
      solTierLamports: Number(view.getBigUint64(44, true)),
      tokenTierBaseUnits: Number(view.getBigUint64(52, true)),
      flatUsdcBaseUnits: Number(view.getBigUint64(60, true)),
      flatEurcBaseUnits: Number(view.getBigUint64(68, true)),
      usdcMint: addressAt(76),
      eurcMint: addressAt(108),
      networkFeeLamports: Number(view.getBigUint64(140, true)),
      networkFeeUsdcBaseUnits: Number(view.getBigUint64(148, true)),
      networkFeeEurcBaseUnits: Number(view.getBigUint64(156, true)),
      feeMode: bytes[164] === 1 ? 'buyer' : 'seller',
      sessionFeeFloat: Number(view.getBigUint64(165, true))
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

  /**
   * `getMinimumBalanceForRentExemption` as a question with a TTL, from the shared
   * `rents.ts` cache — the same answers the allowance page's quotes use, so the
   * two never ask the cluster the same size twice in a minute.
   */
  const rentFor = (space: number) => rentMinimum(client, space)

  /** `getAccountInfo` as a question with a TTL, answered from `accountCache`. */
  async function accountExists(account: Address): Promise<boolean> {
    const cached = accountCache.get(account)
    if (cached && Date.now() - cached.readAt < CACHE_TTL_MS) return cached.exists
    const info = await client.rpc
      .getAccountInfo(account, { commitment: 'confirmed', encoding: 'base64' })
      .send()
    const exists = Boolean(info.value)
    accountCache.set(account, { readAt: Date.now(), exists })
    return exists
  }

  /**
   * Costs for paying `amount` in `currency`. `mint` and `seller` are optional
   * and only used to tell whether a token payment would have to create the
   * seller's token account. `signatures` says how many signatures the path
   * being quoted will carry — 2 when a relayer signs as fee payer, 1 when the
   * wallet sponsors itself — because the policy's network fee is per signature.
   */
  async function estimateInvoiceCosts(
    amount: number,
    currency: SupportedCurrency = 'sol',
    options: { mint?: string, seller?: Address, signatures?: 1 | 2 } = {}
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
      if (!(await accountExists(account))) sellerTokenAccountRent = await rentFor(TOKEN_ACCOUNT_SPACE)
    }

    // The fee leaves for the treasury's own account, which only the deployer
    // can open (`open_treasury_token_account`). Without it no invoice in this
    // mint settles at all, and the buyer would learn that from a raw error
    // after signing — so the quote carries the fact.
    let treasuryTokenAccountMissing = false
    if (isToken && options.mint) {
      const feeAccount = await deriveTreasuryTokenAccount(address(options.mint))
      treasuryTokenAccountMissing = !(await accountExists(feeAccount))
    }

    return invoiceCosts({
      policy,
      amount,
      currency,
      mint: options.mint,
      invoiceRent,
      receiptRent,
      sellerTokenAccountRent,
      treasuryTokenAccountMissing,
      signatures: options.signatures
    })
  }

  return { estimateInvoiceCosts }
}
