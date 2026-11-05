/**
 * SOL prices in the currencies the app can show next to an amount.
 *
 * The rate is a hint — what a price is roughly worth — so a minute-old number
 * is accurate enough. It is cached in the server process and again at the CDN,
 * because every visitor reaching the price API themselves would use up its
 * rate limit for a number that barely moves.
 */
interface SolRates {
  usd: number
  eur: number
  /** When this rate was read, so the UI can say how fresh it is. */
  at: string
}

const TTL_MS = 60_000

let cached: { rates: SolRates, expires: number } | null = null

export default defineEventHandler(async (event) => {
  if (cached && cached.expires > Date.now()) {
    setRateHeaders(event)
    return cached.rates
  }

  // Coinbase quotes SOL in every currency it lists — USD and EUR in one
  // keyless call — so the app never holds an exchange-rate API key.
  const payload = await $fetch<{ data?: { rates?: Record<string, string> } }>(
    'https://api.coinbase.com/v2/exchange-rates?currency=SOL',
    { timeout: 5000 }
  ).catch(() => null)

  const usd = Number(payload?.data?.rates?.USD)
  const eur = Number(payload?.data?.rates?.EUR)
  if (!(usd > 0) || !(eur > 0)) {
    // Without a rate the app shows no fiat hint at all, which is better than
    // a wrong one.
    throw createError({ statusCode: 502, statusMessage: 'No SOL price available right now' })
  }

  const rates: SolRates = { usd, eur, at: new Date().toISOString() }
  cached = { rates, expires: Date.now() + TTL_MS }
  setRateHeaders(event)
  return rates
})

function setRateHeaders(event: Parameters<typeof setHeader>[0]) {
  setHeader(event, 'cache-control', 'public, max-age=60, s-maxage=300, stale-while-revalidate=600')
}
