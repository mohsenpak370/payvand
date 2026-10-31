---
name: vue-solana
description: Build, debug, review, and document Solana apps using @vue-solana/core, @vue-solana/vue, and @vue-solana/nuxt. Use when working with Vue Solana package setup, composables, Nuxt module config, wallet discovery, Android Mobile Wallet Adapter, iOS browser wallets, RPC, balances, transactions, Solana Pay payment requests and QR codes (build, scan, verify settlement), or Solana primitives in Vue/Nuxt apps.
license: MIT
metadata:
  author: vue-solana
---

# Vue Solana

<!-- toc -->
## Contents

- [Package Selection](#package-selection)
- [Install Commands](#install-commands)
- [Vue Setup](#vue-setup)
- [Nuxt Setup](#nuxt-setup)
- [Wallet Flow](#wallet-flow)
- [RPC, Account, And Balance Reads](#rpc-account-and-balance-reads)
- [Data Layer (Request, Subscription, Tracked Data)](#data-layer-request-subscription-tracked-data)
- [Solana Pay](#solana-pay)
- [Sign In With Solana](#sign-in-with-solana)
- [Error Handling](#error-handling)
- [Transactions](#transactions)
- [Client Capabilities, Payer, Identity, And Transaction Planning](#client-capabilities-payer-identity-and-transaction-planning)
- [Common Gotchas](#common-gotchas)
- [Verification Checklist](#verification-checklist)
<!-- /toc -->

Use this skill when helping with apps or libraries that use the Vue Solana ecosystem.

## Package Selection

- Use `@vue-solana/vue/kit` in Vue apps and `@vue-solana/nuxt/kit` in Nuxt apps for Kit primitives (types and values such as `Address`, `Commitment`, `Signature`, `address()`, `lamports()`, and `createSolanaClient()`). Build transaction messages with `@vue-solana/vue/kit` helpers (e.g. `createTransactionMessage()`, `compileTransaction()`); transactions flow through the packages as raw `Uint8Array` wire bytes. The default client is Kit's official stack: `solanaRpc()` installs the transaction planner and the signing/sending executors itself, and `createSolanaClient()` only composes them explicitly when a custom `commitment` is configured. The old custom fallback sender is not used.
- Use `@vue-solana/core` for framework-agnostic config, cluster endpoint helpers, wallet types, Wallet Standard adapters, Android Mobile Wallet Adapter registration, iOS browser wallet helpers, transaction helpers, and core subpath exports.
- Use `@vue-solana/vue` in Vue 3 apps for the plugin and composables.
- Use `@vue-solana/nuxt` in Nuxt apps for module setup and auto-imported composables.

Since v3.0.0 every `@vue-solana/*` package is ESM only (no `require` export condition, no top-level `main`). Never advise adding `@solana/kit` to a consumer's `package.json`: the `kit` subpaths re-export all of it. Four names exist in both Kit and this library — `SolanaError`, `SolanaErrorCode`, `isSolanaError`, `TransactionStatus` — and the package root resolves them to this library's versions, while the `/kit` subpath keeps Kit's. Import `isSolanaError` from `@vue-solana/*/kit` when checking Kit-thrown errors.

- Prefer `devnet` for examples and tests. Use `mainnet` for Solana mainnet; this is Solana's official mainnet cluster name. The legacy `mainnet-beta` spelling is still accepted and redirects to the same endpoint.

## Install Commands

For Vue:

```sh
pnpm add @vue-solana/vue
```

Vue apps do not need `@vue-solana/core` or `buffer` directly for primary composable, Kit primitive, or Buffer-helper usage.

For Nuxt:

```sh
npx nuxt module add @vue-solana/nuxt
```

Nuxt apps do not need `@vue-solana/core`, `@vue-solana/vue`, or `buffer` directly for primary module, composable, Kit primitive, or Buffer-helper usage.

## Vue Setup

Install the plugin once in the app entry:

```ts
import { createApp } from "vue";
import { createSolanaPlugin } from "@vue-solana/vue";
import App from "./App.vue";

const solana = createSolanaPlugin({
  cluster: "devnet",
  mobileWallet: {
    appIdentity: {
      name: "My Vue Solana App",
      uri: "https://example.com",
      icon: "favicon.ico",
    },
  },
});

createApp(App).use(solana).mount("#app");
// app.unmount(); solana.dispose();
```

Prefer direct composable subpath imports in Vue apps:

```ts
import { useAccountInfo } from "@vue-solana/vue/useAccountInfo";
import { useProgramAccounts } from "@vue-solana/vue/useProgramAccounts";
import { useRpc } from "@vue-solana/vue/useRpc";
import { useSignatureStatus } from "@vue-solana/vue/useSignatureStatus";
import { useSolanaPay } from "@vue-solana/vue/useSolanaPay";
import { useSolanaQrCode } from "@vue-solana/vue/useSolanaQrCode";
import { useTransactionConfirmation } from "@vue-solana/vue/useTransactionConfirmation";
import { useWallet } from "@vue-solana/vue/useWallet";
import { useWallets } from "@vue-solana/vue/useWallets";
```

## Nuxt Setup

Register the module in `nuxt.config.ts`:

```ts
export default defineNuxtConfig({
  modules: ["@vue-solana/nuxt"],
  solana: {
    cluster: "devnet",
  },
});
```

Nuxt auto-imports these composables (source of truth: `packages/nuxt/src/imports.ts`):

- `useSolana()`, `useSolanaClient()`
- `useSolanaRpc()`, `useSolanaConnection()`
- `useSolanaWallet()`, `useSolanaWallets()`, `useSolanaSelectedWalletAccount()`
- `useSolanaAccountInfo()`, `useSolanaBalance()`, `useSolanaProgramAccounts()`, `useSolanaSignatureStatus()`, `useSolanaTransactionConfirmation()`, `useSolanaTokenAccounts()`, `useSolanaTokenBalance()`
- `useSolanaRequest()`, `useSolanaSubscription()`, `useSolanaTrackedData()`, `useSolanaAction()`, `useSolanaAirdrop()`
- `useSolanaSignMessage()`, `useSolanaSignIn()`
- `useSolanaSignAndSendTransaction()`, `useSolanaSignTransactions()`, `useSolanaSignAndSendTransactions()`, `useSolanaTransaction()`
- `useSolanaPayer()`, `useSolanaIdentity()`, `useSolanaPlanTransaction()`, `useSolanaPlanTransactions()`, `useSolanaSendTransaction()`, `useSolanaSendTransactions()`
- `useSolanaPay()`, `useSolanaPayConfirmation()`, `useSolanaQrCode()`, `useSolanaQrScanner()`
- `useSolanaClientCapability()`, `tryUseSolana()`, `clearSwrCache()`, `deleteSwrKey()`
- SWR adapters: `useSolanaRequestSwr()`, `useSolanaSubscriptionSwr()`, `useSolanaTrackedDataSwr()`
- Setup values for `solana.clientPlugin: false` (client-only `payer`): `createSolanaPlugin()`, `solanaInjectionKey()`, `selectedWalletAccountInjectionKey()`, `createSelectedWalletAccountContext()`

`useClientCapability` / `useSolanaClientCapability()` fail fast when the client lacks a capability (throws `MissingClientCapabilityError`, normalized to `SolanaError`). In Nuxt it's auto-imported as `useSolanaClientCapability()`; in plain Vue import from `@vue-solana/vue/useClientCapability` or rely on hooks that call it internally. `tryUseSolana()` returns the context or `null` without creating an SSR fallback. `createSolanaPlugin()` returns `dispose()` to stop wallet discovery/subscriptions; Nuxt wires it automatically.

The Nuxt runtime plugin is client-only. Composables are SSR-safe and may return inert state during SSR; run real RPC and wallet work after hydration, in client lifecycle hooks, or from user actions. The one exception is `useSolanaQrScanner()`: never call `start()` during SSR, and wrap scanner UI in `<ClientOnly>` (see Solana Pay).

Nuxt `ModuleOptions` intentionally omits `payer` and `payerSecretKey`. Direct `@vue-solana/core` and `@vue-solana/vue` clients accept both: `payer` is a Kit `TransactionSigner`, and `payerSecretKey` is a base64 64-byte Ed25519 keypair. Never put a raw secret or `payerSecretKey` in Nuxt public runtime config. Install a client-owned `payer` in a client-only plugin with `clientPlugin: false` so the module does not install a second plugin; a client-sent transaction always needs one.

## Wallet Flow

Use one public wallet flow for all supported wallet sources:

1. Use `useWallets()` or `useSolanaWallets()` to discover wallets and select one. Connecting a wallet disconnects every other discovered wallet (swept over discovery; uncached wallets disconnected via `standard:disconnect` with a 2 s timeout), so only one wallet stays authorized at a time. Per-wallet failures are swallowed and `connect()` resolves after the sweep.
2. Use `useWallet()` or `useSolanaWallet()` for active wallet state.
3. Call `connect()` only after selecting a wallet.
4. Treat `connected` as false until `connect()` resolves, even if an extension exposes previously authorized accounts.
5. Call `disconnect()` from the active wallet composable.

To persist a wallet choice across sessions, mount `SelectedWalletAccountProvider` once near the app root and use `useSelectedWalletAccount()` (or `useSolanaSelectedWalletAccount()` in Nuxt) to read and set the selected account. The Nuxt runtime plugin installs an app-wide selected-wallet-account context automatically, so `useSolanaSelectedWalletAccount()` works without a provider. The selection persists as `${walletName}:${accountAddress}` through `stateSync` (default `localStorage`), syncs across tabs, and is restored when the wallet reconnects.

Current wallet support:

- Browser extension wallets discovered through Solana Wallet Standard packages.
- Android native mobile wallets through `@solana-mobile/wallet-standard-mobile` on Android Chrome and Chrome PWAs.
- iOS browser wallets through wallet-specific universal links for Phantom, Solflare, and Backpack.
- Manual or custom wallet objects that implement `SolanaWallet`.

Current wallet limits:

- Desktop native app wallets are not implemented yet. They require wallet-specific protocol links or future native Wallet Standard registration.
- There is no built-in wallet modal. Apps should build their own selection UI with `useWallets()`.
- `autoConnect` is active behavior only when explicitly enabled. It may reconnect a restored, previously selected wallet, but must never be used to connect an arbitrary installed wallet.

## RPC, Account, And Balance Reads

Use `useRpc()` or `useSolanaRpc()` to show cluster, endpoint, connection status, latest blockhash, and `checkConnection()`.

Use `useBalance(address, commitment?)` or `useSolanaBalance(address, commitment?)` for lamport balances. The address is a base58 address string (or a ref/getter of one); pass `null` or `undefined` to disable the read.

Use `useAccountInfo(address, options?)` or `useSolanaAccountInfo(address, options?)` to read account data and optionally subscribe to account changes.

Use `useProgramAccounts(programId, config?)` or `useSolanaProgramAccounts(programId, config?)` for program-owned account scans. Program scans can be expensive on public RPC nodes; prefer narrow filters, `dataSlice`, and dedicated RPC infrastructure for production reads.

Use `useSignatureStatus(signature, options?)` or `useSolanaSignatureStatus(signature, options?)` to read or poll submitted transaction status. Polling uses chained timeouts; `stopPolling()` clears the timer and polling restarts when the signature changes. There is no per-call `commitment` option (`getSignatureStatuses` takes none) — set commitment on the endpoint or plugin. For per-call commitment, use `useTransactionConfirmation()`.

RPC, balance, account, program-account, and signature-status reads do not require a connected wallet.

## Data Layer (Request, Subscription, Tracked Data)

Use the Kit-reactive data composables for SWR-style reads instead of hand-rolled `watch` + `try/catch` state:

- `useRequest(source, options?)` or auto-imported `useSolanaRequest(source, options?)` for one-shot requests. The source may be a request function, a Kit request object (`rpc.getBalance(address)`), a `ref`/`computed` of either, or `null` to disable. Statuses: `fetching`, `success`, `error`, `disabled`.
- `useSubscription(source, options?)` or `useSolanaSubscription(...)` for live streams. The source is a Kit reactive stream source (for example `rpcSubscriptions.slotNotifications()`), a ref of it, or `null`. Statuses: `loading`, `loaded`, `error`, `disabled`. `reconnect()` re-opens with stale-while-revalidate.
- `useTrackedData(source, options?)` or `useSolanaTrackedData(...)` for RPC data seeded by a fetch and updated by a subscription (for example `getAccountInfo` + `accountNotifications`). The store slot-deduplicates both sources so out-of-order arrivals cannot regress the value.
- SWR cache adapters from `@vue-solana/vue/swr` (`useRequestSwr`, `useSubscriptionSwr`, `useTrackedDataSwr`) key results across mounts. Keys are namespaced per adapter; a `null` source clears the key's cache. Cache is capped at 1024 entries with oldest-first eviction; use `clearSwrCache()` or `deleteSwrKey(key)` (full key includes the adapter prefix). In Nuxt these are auto-imported as `useSolanaRequestSwr()`, `useSolanaSubscriptionSwr()`, `useSolanaTrackedDataSwr()`, plus `clearSwrCache()` and `deleteSwrKey()`; in plain Vue import from `@vue-solana/vue/swr`.
- `useAction(handler, options?)` or `useSolanaAction(handler, options?)` for generic async actions, backed by `createSolanaActionStore` from `@vue-solana/core/action`. The handler receives a fresh `AbortSignal` per call. Exposes `data`, `dispatch`, `error`, `isRunning`, `reset`, and `status` (`idle`, `running`, `success`, `error`). Re-dispatch aborts the in-flight call; failures keep the previous `data` for stale-while-revalidate.

For `useTrackedData`, the `rpcValueMapper` / `rpcSubscriptionValueMapper` callbacks receive the unwrapped value; the returned `data` ref keeps the full `SolanaRpcResponse` envelope (`data.value.value`, `data.value.context.slot`).

Tests touching these composables or real Kit reactive stores must run under the `node` vitest environment (Kit stores call Node's `setMaxListeners` on `AbortSignal`); tag the test file with `// @vitest-environment node`.

## Solana Pay

Solana Pay is implemented directly on Kit. No extra Solana Pay package is required, and `qrcode` is loaded lazily only when a QR code is rendered.

Core entry points:

- `@vue-solana/core/solana-pay`: `encodeURL()`, `encodeTransferRequestURL()`, `encodeTransactionRequestURL()`, `parseURL()`, `createQR()`, `createQRDataURL()`, `createQRCanvas()`, `createQROptions()`, and `ParseURLError`.
- `@vue-solana/core/solana-pay-confirmation`: `createSolanaPayReference()`, `verifySolanaPayPayment()`, `findTransactionSignaturesByReference()`.
- `@vue-solana/core/solana-pay-status`: `getSolanaPayPaymentStatus()`.

These helpers live in `@vue-solana/core`. In a Vue or Nuxt app prefer the composables below; if you call the helpers directly, add `@vue-solana/core` as a direct dependency, because a transitive copy is not resolvable from app source under pnpm's strict `node_modules`.

`encodeTransactionRequestURL()` only builds the URL: the linked HTTPS endpoint, its authorization, and the transaction it returns are the app's responsibility, and this library never fetches or signs the linked transaction.

Amounts are `bigint` lamports (`100_000_000n` encodes `amount=0.1`), while `recipient`, `splToken`, and `reference` are Kit `Address` values. A `reference` is a lookup key, never proof of payment on its own.

Vue composables (Nuxt auto-imports all four):

- `useSolanaPay()`: `createTransferRequest({ recipient, amount, splToken?, label?, message?, memo? })` mints a reference, returns the URL string, and fills `requestUrl`, `parsedRequest`, `reference`, `status` (`idle`, `pending`, `found`, `confirmed`, `finalized`, `expired`, `error`), `loading`, and `error`. `parseRequest(value)` parses untrusted input, `createReference()` mints a standalone reference, `confirmTransfer(options?)` verifies against the parsed request and stores the outcome, and `reset()` clears state.
- `useSolanaPayConfirmation()`: `startPolling(rpc, reference, 2_000)` / `stopPolling()` plus `check(rpc, reference)` over the cheaper non-verifying `getSolanaPayPaymentStatus()`. Pass the `rpc` surface from `useSolanaClient()`; polling stops on unmount.
- `useSolanaQrCode()`: `generate(url, { size, background, color })` resolves an SVG string (render with `v-html`; the markup comes from the encoder, not user input) and `reset()` clears it. The option values are forwarded positionally to `createQR(url, size, background, color)`, so pass all three keys or none — `{ size: 320 }` alone is fine, but `{ size: 320, color: "#000" }` would put the color in the background slot. Use `createQRDataURL()` for a percent-encoded `data:` URL that needs no `Buffer` polyfill.
- `useSolanaQrScanner()`: camera plus `BarcodeDetector` scanning. `start({ video, scanIntervalMs })`, `stop()`, `reset()`, and `scanFromText(text)` for a paste fallback. It is browser-only: never call `start()` during SSR, and wrap scanner UI in `<ClientOnly>` in Nuxt. When `BarcodeDetector` is unavailable the camera still opens but nothing is detected, so always offer a paste field.

Security boundaries: parse every scanned or pasted URL before displaying it; show recipient, amount, token, and memo and require an explicit user action before any wallet prompt; never auto-send; fulfil only when `verifySolanaPayPayment()` returns `verified: true`; never store private keys or secrets in client code or Nuxt public runtime config.

The example apps' `SolanaPayPanel` (request, QR, status polling, settlement verification) and `SolanaPayScannerPanel` (scan or paste, parsed preview, wallet handoff with no send path) are runnable references.

## Sign In With Solana

Use `useSignIn()` or `useSolanaSignIn()` to trigger a wallet's SIWS feature. `signIn(input?)` resolves `{ account, signedMessage, signature }` for **server-side verification**: verify the signature against `account.publicKey` on your backend before trusting the identity (see the message-signing guide). The adapter verifies message/sign-in signatures against the account's public key (best-effort, rejects malformed signatures even if Ed25519 unavailable) and binds sign-in results to the requested account (rejects mismatch/unknown accounts with `INVALID_SIGNATURE`). State exposes `status` (`idle`, `signing-in`, `signed-in`, `error`), `loading`, `error`, and `signInResult`; wallet cancellations normalize to `USER_REJECTED`.

The composable does not auto-connect: `signIn()` rejects with `WALLET_NOT_CONNECTED` when no wallet is connected.

## Error Handling

Vue Solana normalizes common failures into `SolanaError` from `@vue-solana/core/errors`. Public examples and docs should branch on stable `error.code` values instead of rendering raw error objects or parsing wallet/RPC messages.

Stable codes:

- `NO_WALLET_SELECTED`
- `WALLET_NOT_CONNECTED`
- `WALLET_FEATURE_UNSUPPORTED`
- `WALLET_FAILURE`
- `USER_REJECTED`
- `INVALID_ADDRESS`
- `INVALID_INPUT`
- `TRANSACTION_TIMEOUT`
- `TRANSACTION_NOT_FOUND`
- `TRANSACTION_FAILED`
- `REDIRECT_TIMEOUT`
- `DECRYPTION_FAILED`
- `RPC_FAILURE`
- `STORAGE_FAILURE`
- `MISSING_CLIENT_CAPABILITY`
- `INVALID_SIGNATURE`

Use `error.cause` only for debugging/logging the original wallet adapter, RPC, parsing, timeout, or storage failure. Do not show raw `cause` details to users by default.

Vue composables expose `Ref<SolanaError | null>` error refs. Nuxt auto-imported composables expose the same error shape.

```ts
const message = computed(() => {
  switch (error.value?.code) {
    case "NO_WALLET_SELECTED":
      return "Choose a wallet first.";
    case "USER_REJECTED":
      return "The wallet request was rejected.";
    case "TRANSACTION_TIMEOUT":
      return "The transaction is taking longer than expected.";
    case "RPC_FAILURE":
      return "The Solana RPC request failed.";
    default:
      return null;
  }
});
```

## Transactions

Use `useSignAndSendTransaction()` or `useSolanaSignAndSendTransaction()` after a wallet is selected and connected. This wallet flow can return after RPC submission or wait for a selected commitment with `confirm: true`.

Use `useSendTransaction()` / `useSendTransactions()` (and their Nuxt aliases) for client-owned signing. The default client exposes Kit's official planner and signing/sending capabilities (`solanaRpc()` installs them; `createSolanaClient()` re-adds them explicitly for a custom `commitment`). The official executor waits for `confirmed` commitment before the composable reports `sent`, with no wallet popup. Configure `payer`, and keep funded keys in a trusted server or relayer context.

The active wallet must support either `signAndSendTransaction` or `signTransaction`. Android Mobile Wallet Adapter wallets prefer `signTransaction` plus app-side RPC submission when available so the app can reliably return the submitted signature.

Use `useTransactionConfirmation()` or `useSolanaTransactionConfirmation()` when an app already has a submitted signature and wants to wait for a requested commitment separately from signing and sending.

Use `useTransaction(handler, { timeoutMs? })` or `useSolanaTransaction()` to track `{ signature, loading, error }` around a custom transaction builder. A newer execution supersedes an in-flight one, and a timeout normalizes to `TRANSACTION_TIMEOUT`.

Use `useAirdrop()` or `useSolanaAirdrop()` on devnet/testnet/local validators to dispatch `(address, lamports)`. It requires the client's `airdrop` capability (installed by `rpcAirdrop()` in the default client) and a `payer` signer. Rate-limited or drained faucets normalize to `RPC_FAILURE` with an actionable message; point users at `https://faucet.solana.com` or a local validator.

For batch flows, use `useSignTransactions()` / `useSolanaSignTransactions()` (sign multiple transactions in one wallet request) and `useSignAndSendTransactions()` / `useSolanaSignAndSendTransactions()` (sign and send a batch). Both prefer the wallet's batch capability: `useSignTransactions` falls back to the legacy `signAllTransactions` feature, while `useSignAndSendTransactions` falls back to sending sequentially with the singular capability. A partial batch send rejects with `PartialSignAndSendError` (from `@vue-solana/vue/useSignAndSendTransactions`); its `signatures` array lists the transactions already submitted, so inspect it before retrying to avoid double-spends.

When browser transaction code needs `Buffer`, use the framework package helper:

```ts
import { installSolanaBufferPolyfill } from "@vue-solana/vue/buffer-polyfill";

installSolanaBufferPolyfill();
```

Use `@vue-solana/nuxt/buffer-polyfill` for the same helper in Nuxt apps. Use `@vue-solana/core/buffer-polyfill` only for framework-agnostic core usage.

Do not assign the Buffer global manually in public examples.

Since 4.0.0 the root barrels no longer re-export `Buffer` or `installSolanaBufferPolyfill`; always import the `buffer-polyfill` subpath so the polyfill is not pulled into every consumer's initial chunk.

## Client Capabilities, Payer, Identity, And Transaction Planning

- `useClientCapability(methods, options?)` / `useSolanaClientCapability()` fail fast when the installed Solana client does not expose a named capability, throwing `MissingClientCapabilityError` (extends `SolanaError`, code `MISSING_CLIENT_CAPABILITY`). Auto-imported in Nuxt as `useSolanaClientCapability()`. `tryUseSolana()` returns the context or `null` without creating an SSR fallback; `useSolana()` returns a fresh inert SSR context per call if uninstalled. `createSolanaPlugin()` returns `dispose()` to stop wallet discovery/subscriptions (Nuxt wires automatically).
- `usePayer()` / `useIdentity()` (or `useSolanaPayer()` / `useSolanaIdentity()`) expose the Kit client's `payer` / `identity` `TransactionSigner`, re-read on change when the client advertises `subscribeToPayer` / `subscribeToIdentity`. A direct Vue/core client can receive `payer` through plugin options; custom clients can install signer plugins (e.g. `generatedPayer()` / `generatedIdentity()` from `@solana/kit-plugin-signer`). Nuxt does not forward payer options through public runtime config.
- `usePlanTransaction()` / `usePlanTransactions()` (or `useSolanaPlanTransaction()` / `useSolanaPlanTransactions()`) expose the client's transaction planning capability. `usePlanTransaction` requires `planTransaction` and `payer`; `usePlanTransactions` requires `planTransactions` and `payer`. Kit reads `client.payer` to set the fee payer, so a client-sent transaction always needs one.
- `useSendTransaction()` / `useSendTransactions()` (or `useSolanaSendTransaction()` / `useSolanaSendTransactions()`) plan, sign, submit, and confirm through the client's sending executor. Same rule: they require `sendTransaction`/`sendTransactions` **and** `payer`.

## Common Gotchas

- Do not import Solana primitives from `@solana/web3.js`, `@solana/web3-compat`, or the removed `@vue-solana/*/web3` subpaths in new Vue Solana examples; use `@vue-solana/vue/kit`, `@vue-solana/nuxt/kit`, or `@vue-solana/core/kit` for the Kit primitive surface.
- v2.0.0 removed `@solana/web3-compat` from every package, deleted the `web3` subpaths, dropped the `connection` field from the context, and removed the declaration shims v1 published for the broken `web3-compat` metadata. Do not suggest local `types/web3-compat.d.ts` shims; upgrade examples to `@vue-solana/*@^2` instead.
- Do not split browser, Android mobile, iOS browser, and future desktop native wallet sources into separate public flows. Keep them unified through `useWallets()` and `useWallet()`.
- Do not mark a discovered wallet as connected just because accounts are visible. Connection state begins after `connect()` succeeds.
- In Nuxt, avoid server-side RPC and wallet actions unless the app explicitly provides server-safe behavior. Keep `payerSecretKey` and all raw secrets out of public runtime config; create client-owned signers in a client-only plugin and set `clientPlugin: false`.
- Public Solana RPC endpoints can be rate-limited. For production, suggest a dedicated RPC provider and custom `endpoint`.
- When example-app behavior changes, extend the Playwright e2e suite (`e2e/`) rather than relying on unit tests alone. `mockSolanaRpc(page)` answers the reads the examples use (including `getSignaturesForAddress` and `getTransaction` for Solana Pay settlement), and `mockSolanaSubscriptions(page)` fakes the Kit RPC-subscriptions websocket protocol (subscribe, id-correlated result, then `<method>Notification` frames keyed on `params.subscription`); see the E2E Testing guide. Never depend on a real camera in CI: the scanner spec stubs `navigator.mediaDevices.getUserMedia` with a canvas `captureStream()` and a fake `BarcodeDetector`.

## Verification Checklist

For changes inside the Vue Solana repository, run these from the repo root:

```sh
pnpm format
pnpm lint
pnpm test
pnpm typecheck
pnpm build:packages
pnpm build:examples
pnpm --filter docs build
pnpm smoke:standalone-installs
pnpm test:e2e
```

After touching a barrel, `exports` map, or `package.json` `files`, also run `pnpm check:entries`, `pnpm check:esm-map`, `pnpm check:externals`, `pnpm check:buffer-polyfill-declarations`, `pnpm size:packages`, and `pnpm pack:size`.

For consumer app examples, verify the app starts and the relevant flow works on `devnet`. For wallet or transaction work, include manual browser testing with a Solana wallet when feasible. For Solana Pay settlement, note that the automated suite only covers the no-payment (`expired`) branch; confirm a real `confirmed` result manually with a wallet.
