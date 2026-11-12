// https://nuxt.com/docs/api/configuration/nuxt-config
import { readFileSync, readdirSync } from 'node:fs'

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

// Every markdown file under content/<locale>/learn is a route. Both locales
// use the same slugs, so the English set also lists the Persian guides. Nitro's
// crawler does not follow the card links on the index page, so the guides are
// listed here and prerendered as static HTML — which also keeps the content
// database out of the deployed server function's request path.
// The release number is the workspace root's version, read at build time so the
// footer cannot drift from it: bumping package.json is the whole of a release.
const { version } = JSON.parse(
  readFileSync(new URL('../../package.json', import.meta.url), 'utf8')
) as { version: string }

const learnGuides = readdirSync(new URL('./content/en/learn', import.meta.url))
  .filter(file => file.endsWith('.md'))
  .map(file => `/learn/${file.replace(/\.md$/, '')}`)

export default defineNuxtConfig({
  // `@nuxtjs/i18n` has to come before `@nuxt/ui` in this list: the UI module
  // looks for it during setup to hand its components the locale messages
  // ("Open menu", "Switch to dark mode", …), and a module registered later is
  // not there yet, which leaves those labels in English.
  modules: ['@nuxt/eslint', '@nuxtjs/i18n', '@nuxt/ui', '@nuxt/content', '@vite-pwa/nuxt', '@vue-solana/nuxt'],

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
      cluster: solanaCluster,
      version
    }
  },

  routeRules: {
    '/': { prerender: true },
    // The guides are markdown compiled at build time, so there is nothing per
    // request to render: ship them as static pages and keep the content
    // database out of the server function entirely.
    '/learn': { prerender: true },
    '/learn/**': { prerender: true },
    // Legal pages are the same text for everyone and change only when the app
    // is deployed, so they ship as static HTML too.
    '/terms': { prerender: true },
    '/privacy': { prerender: true },
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
    // The currency and the shops switch are browser preferences, so the server
    // has nothing to render — and would otherwise paint today's default first.
    '/settings': { ssr: false },
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
      // The account's own on-chain history, read by /profile. Its own prefix:
      // it is not under /api/auth, so without this rule the browser's call
      // falls through to Nitro and comes back "Page not found".
      '/api/me': {
        target: 'http://127.0.0.1:8787/api/me',
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
      // CommonJS, loaded only in the browser. `qrcode` is not imported by this
      // app: `useSolanaQrCode` pulls it in with a dynamic import from
      // @vue-solana/core, and without pre-bundling Vite serves that file raw,
      // where `require` does not exist and the QR never appears.
      include: ['qrcode']
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

  // Every word the app shows lives in i18n/locales/<locale>.json; nothing is
  // written into a template, so a second language is a second file and no
  // component changes. `no_prefix` keeps the URLs exactly as they are: these
  // are QR targets and wallet hand-offs, and an /en/ in front of an invoice
  // link would break every code already printed or shown.
  i18n: {
    // URLs never carry the locale: payment links are permanent and printed QR
    // codes point at them, so a shared link must read the same to everyone.
    strategy: 'no_prefix',
    defaultLocale: 'en',
    locales: [
      { code: 'en', language: 'en-US', name: 'English', file: 'en.json' },
      { code: 'fa', language: 'fa-IR', name: 'فارسی', dir: 'rtl', file: 'fa.json' },
      { code: 'fr', language: 'fr-FR', name: 'Français', file: 'fr.json' },
      { code: 'de', language: 'de-DE', name: 'Deutsch', file: 'de.json' }
    ],
    // The choice made on the settings page is kept in a cookie so it survives
    // a reload, and a first visit follows the browser's own language.
    detectBrowserLanguage: {
      useCookie: true,
      cookieKey: 'payvand.locale'
    }
  },

  // Installable app: the module emits the web manifest, links it from every
  // page and registers a service worker that precaches the build assets.
  // `autoUpdate` keeps the installed app on the current release; the worker
  // deliberately caches no navigation responses, because an invoice or a
  // payment status read from a cache would be a stale one.
  pwa: {
    // A new deployment installs a new service worker. Taking over immediately
    // reloads the page under whoever is on it, which on the checkout can land
    // mid-payment; the update waits instead and app/plugins/update.client.ts
    // offers it.
    registerType: 'prompt',
    manifest: {
      id: '/',
      // The launch splash prints this under the icon and truncates anything
      // long, so it is the name alone; the tagline lives in `description`.
      name: 'Payvand',
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
