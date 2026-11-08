// https://nuxt.com/docs/api/configuration/nuxt-config
import { readdirSync } from 'node:fs'

// The cluster the app targets is set in `.env` (`SOLANA_CLUSTER`), not here:
// 'localnet' for a local `solana-test-validator`, 'devnet' for testing,
// 'mainnet' for real payments. Nuxt's CLI loads `.env` before this file is
// evaluated; without the variable we fall back to devnet.
type SolanaCluster = 'localnet' | 'devnet' | 'testnet' | 'mainnet'
const SOLANA_CLUSTERS: readonly string[] = ['localnet', 'devnet', 'testnet', 'mainnet']
const rawCluster = process.env.SOLANA_CLUSTER || 'devnet'
if (!SOLANA_CLUSTERS.includes(rawCluster)) {
  throw new Error(`SOLANA_CLUSTER=${rawCluster} is not one of: ${SOLANA_CLUSTERS.join(', ')}`)
}
const solanaCluster = rawCluster as SolanaCluster

// The module has no built-in endpoint for localnet, so point it at the
// validator explicitly. An explicit NUXT_PUBLIC_SOLANA_ENDPOINT always wins.
const solanaEndpoint
  = process.env.NUXT_PUBLIC_SOLANA_ENDPOINT
    || (solanaCluster === 'localnet' ? 'http://127.0.0.1:8899' : undefined)

// Every markdown file under content/learn is a route. Nitro's crawler does not
// follow the card links on the index page, so the guides are listed here and
// prerendered as static HTML — which also keeps the content database out of
// the deployed server function's request path.
const learnGuides = readdirSync(new URL('./content/learn', import.meta.url))
  .filter(file => file.endsWith('.md'))
  .map(file => `/learn/${file.replace(/\.md$/, '')}`)

export default defineNuxtConfig({
  modules: ['@nuxt/eslint', '@nuxt/ui', '@nuxt/content', '@vite-pwa/nuxt', '@vue-solana/nuxt'],

  devtools: {
    enabled: true
  },

  css: ['~/assets/css/main.css'],

  runtimeConfig: {
    public: {
      // Empty means same origin. In dev Nitro proxies /api to the Axum
      // server below; in production set NUXT_PUBLIC_API_BASE to the API's
      // origin (it is deployed as its own Vercel project).
      apiBase: '',
      cluster: solanaCluster
    }
  },

  routeRules: {
    '/': { prerender: true },
    // The guides are markdown compiled at build time, so there is nothing per
    // request to render: ship them as static pages and keep the content
    // database out of the server function entirely.
    '/learn': { prerender: true },
    '/learn/**': { prerender: true },
    // Both flows talk to a connected wallet and a camera, so they only make
    // sense in the browser. The Solana plugin is client-only.
    '/sell': { ssr: false },
    // The checkout is the scan target now, and it is as client-only as /pay:
    // it reads the invoice from the URL, the wallet and the allowance.
    '/p/**': { ssr: false },
    '/pay': { ssr: false },
    '/scan': { ssr: false },
    // Budget state lives in the wallet + localStorage, same reason.
    '/budget': { ssr: false },
    // The allowance card reads the chain through the client-only Solana
    // plugin (useSolanaClient throws during SSR) and the session itself
    // lives in localStorage — there is nothing to render on the server.
    '/profile': { ssr: false }
  },

  // Listen on every interface so a phone on the same network can open the
  // app for testing (port 3001: 3000 is taken by another local project).
  // HTTPS matters beyond politeness: browsers only expose the camera
  // (getUserMedia) in a secure context, so plain http:// on the LAN IP
  // cannot scan QR codes.
  devServer: {
    host: '0.0.0.0',
    port: 3001,
    https: {
      key: './.certs/dev-key.pem',
      cert: './.certs/dev.pem'
    }
  },

  compatibilityDate: '2026-06-30',

  nitro: {
    // Prerendered guide pages, one per markdown file in content/learn.
    prerender: {
      routes: ['/learn', ...learnGuides]
    },

    // `pnpm dev` forwards API calls to the local Axum server. The prefixes
    // are listed explicitly instead of a single `/api` rule: Nuxt Icon also
    // serves its collections from `/api/_nuxt_icon/*`, and a blanket `/api`
    // proxy would swallow those requests and force the CDN fallback.
    devProxy: {
      '/api/auth': {
        target: 'http://127.0.0.1:8787/api/auth',
        changeOrigin: true
      },
      '/api/payments': {
        target: 'http://127.0.0.1:8787/api/payments',
        changeOrigin: true
      },
      // Wallets ask the API for a transaction; in dev the QR points at this
      // origin, so forward that request to the Axum server.
      '/api/solana-pay': {
        target: 'http://127.0.0.1:8787/api/solana-pay',
        changeOrigin: true
      },
      '/api/saved-payments': {
        target: 'http://127.0.0.1:8787/api/saved-payments',
        changeOrigin: true
      },
      '/api/discounts': {
        target: 'http://127.0.0.1:8787/api/discounts',
        changeOrigin: true
      },
      '/api/waitlist': {
        target: 'http://127.0.0.1:8787/api/waitlist',
        changeOrigin: true
      },
      // The seller's shops and their products, read by /profile, the shop
      // pages and /sell.
      '/api/shops': {
        target: 'http://127.0.0.1:8787/api/shops',
        changeOrigin: true
      },
      '/api/health': {
        target: 'http://127.0.0.1:8787/api/health',
        changeOrigin: true
      },
      // Admin routes, so /admin and the header's account menu work in dev
      // the same way they do against the deployed API.
      '/api/admin': {
        target: 'http://127.0.0.1:8787/api/admin',
        changeOrigin: true
      }
    }
  },

  vite: {
    optimizeDeps: {
      // All three are CommonJS and only load in the browser. `qrcode` is not
      // imported by this app: `useSolanaQrCode` pulls it in with a dynamic
      // import from @vue-solana/core, and without pre-bundling Vite serves
      // that file raw, where `require` does not exist and the QR never
      // appears.
      include: ['jsqr', 'qrcode']
    }
  },

  eslint: {
    config: {
      stylistic: {
        commaDangle: 'never',
        braceStyle: '1tbs'
      }
    }
  },

  // Installable app: the module emits the web manifest, links it from every
  // page and registers a service worker that precaches the build assets.
  // `autoUpdate` keeps the installed app on the current release; the worker
  // deliberately caches no navigation responses, because an invoice or a
  // payment status read from a cache would be a stale one.
  pwa: {
    registerType: 'autoUpdate',
    manifest: {
      id: '/',
      name: 'Payvand — Solana QR payments',
      short_name: 'Payvand',
      description: 'Take payment with a QR code, or scan one and pay from your wallet.',
      lang: 'en',
      categories: ['finance', 'business'],
      start_url: '/',
      scope: '/',
      display: 'standalone',
      // Matches the logo's navy, so the launch splash and the app window share
      // one colour instead of flashing white before the dark theme loads.
      theme_color: '#080E20',
      background_color: '#080E20',
      icons: [
        { src: '/pwa-192x192.png', sizes: '192x192', type: 'image/png' },
        { src: '/pwa-512x512.png', sizes: '512x512', type: 'image/png' },
        // Full-bleed navy so launchers that crop icons to a circle keep the
        // mark intact.
        { src: '/pwa-maskable-512x512.png', sizes: '512x512', type: 'image/png', purpose: 'maskable' }
      ]
    },
    workbox: {
      globPatterns: ['**/*.{js,css,png,svg,ico,woff2}'],
      cleanupOutdatedCaches: true
    }
  },

  solana: {
    cluster: solanaCluster,
    autoConnect: true,
    ...(solanaEndpoint ? { endpoint: solanaEndpoint } : {})
  }
})
