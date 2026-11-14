import { getAddressDecoder, type Address } from '@vue-solana/nuxt/kit'
import { accountDiscriminator, deriveConfigPda } from './usePaymentProgram'

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
