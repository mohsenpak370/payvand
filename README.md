# Payvand

A two-sided Solana payment app.

- **Seller** connects a wallet (they never sign), enters an amount, and the app
  derives the invoice address locally and renders a QR code — nothing touches
  the chain until a customer pays.
- **Buyer** scans the code with a Solana wallet (or the app's own scanner),
  sees the amount and memo, and approves a single `settle_payment`
  transaction that opens the invoice (`Payment` PDA) and pays it in one
  signature.
- The **Axum API** validates payment requests and reports live on-chain status.

An account is required before either side can be used. Sign-up and sign-in are
email and password today; `users.wallet` is already in the schema so the same
account can later also sign in with a Solana wallet.

## Stack

| Part      | Tech                                                       |
| --------- | ---------------------------------------------------------- |
| Frontend  | Nuxt 4, Nuxt UI v4, Tailwind CSS 4, [@vue-solana/nuxt][vs] |
| QR        | `qrcode` (generate), `jsqr` (camera scan)                  |
| Backend   | Rust, Axum 0.8, `solana-rpc-client`, libSQL (`users`, `sessions`) |
| Solana    | Anchor 1.1.2 program (`apps/contracts/programs/payvand`)   |
| Hosting   | Vercel (web project + API project)                         |

[vs]: https://vue-solana-docs.vercel.app/packages/nuxt

## Repository layout

```
apps/web/            Nuxt app (run `pnpm dev` from the repo root)
  app/pages/sell.vue     Seller: create invoice + render QR (requires an account)
  app/pages/pay.vue      Buyer: the shared, printable payment page (name, price, QR)
  app/pages/scan.vue     Buyer: scan a QR in the app and pay from this device
  app/pages/signin.vue   Email + password sign-in
  app/pages/signup.vue   Create an account
  app/assets/payvand.idl.json   Copy of the Anchor IDL used by the client
apps/api/            Axum API (also the Vercel Rust function)
apps/contracts/      Anchor program + workspace Cargo.toml/Anchor.toml
  programs/payvand/      The on-chain program
  target/                Anchor build output — includes the program keypair
plans/               Implementation plans (numbered; templates + completed/)
Taskfile.yml         Task runner entry points (`task --list`)
```

The web app is a standalone pnpm project in `apps/web` (its own lockfile);
the repo-root `package.json` only forwards the documented commands into it.

## Prerequisites

- Node 20+ and pnpm
- Rust (stable) and Cargo
- [Solana CLI][solana] and [Anchor CLI 1.1.2][anchor]
- A browser wallet (Phantom, Solflare, Backpack, …)

[solana]: https://docs.solanalabs.com/cli/install
[anchor]: https://www.anchor-lang.com/docs/installation

## Setup

```bash
pnpm install               # installs the Nuxt app and runs `nuxt prepare`
pnpm anchor:build          # builds the program and syncs the IDL into app/assets
pnpm anchor:deploy         # deploys to the cluster in Anchor.toml (devnet by default)
```

`pnpm anchor:build` copies `apps/contracts/target/idl/payvand.json` to
`apps/web/app/assets/payvand.idl.json`; commit that copy — the browser reads the
discriminators and program address from it.

> The program keypair lives in `apps/contracts/target/deploy/payvand-keypair.json` and is
> gitignored, like every Anchor project. Back it up or you lose upgrade rights
> over the deployed program.

### Program and clusters

| | |
| --- | --- |
| Program ID | `4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J` |
| Deployed on | **devnet** (upgrade authority: the deploy wallet) |
| Cluster config | `Devnet` in `apps/contracts/Anchor.toml`, `SOLANA_CLUSTER` in `apps/web/.env` |

Wallet addresses, deploy costs and funding routes live in `DEV.md`
(gitignored — they go stale; regenerate with `solana address`).

The ID is registered in three places that must always agree:
`declare_id!` in `apps/contracts/programs/payvand/src/lib.rs`, `[programs.*]`
in `apps/contracts/Anchor.toml`, and the `address` field of
`apps/web/app/assets/payvand.idl.json` (the
client takes its program address from that IDL copy). `anchor keys list`
prints what the keypair on disk actually is; if it disagrees with
`declare_id!`, run `anchor keys sync` followed by `pnpm anchor:build`.

Check what is live on a cluster:

```bash
solana account 4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J --url devnet
# look for "Executable: true"
```

### Local development

Run the API and the web app in two terminals:

```bash
pnpm api        # Axum on http://127.0.0.1:8787
pnpm dev        # Nuxt on http://localhost:3000
```

The same commands — plus `verify`, `build`, the Anchor tasks and the
production deploys — are defined in `Taskfile.yml`; run `task --list` to see
them all.

In dev, Nitro proxies `/api/**` to the Axum server, so no CORS setup or env
file is needed. The API also runs with no database configured at all: it falls
back to an embedded SQLite file at `data/payvand.db` and migrates the schema
on startup.

The cluster the app targets lives in one place: `SOLANA_CLUSTER` in
`apps/web/.env` (`localnet`, `devnet`, `testnet` or `mainnet`, default
`devnet`). Nuxt reads
it when the config loads and the API reads it on startup, so change it there
and restart `pnpm dev` / `pnpm api` — never in code. The deploy cluster is
different: Anchor does not read `.env`, so `pnpm anchor:deploy` still follows
`[provider] cluster` in `apps/contracts/Anchor.toml`. To run fully offline
against a local cluster, start `solana-test-validator`, set
`SOLANA_CLUSTER=localnet` in `apps/web/.env` and the `[provider]` cluster in
`Anchor.toml` to `Localnet`.

Useful endpoints once the API is up:

```
GET  /health                 -> {"ok":true}
POST /api/auth/signup        -> creates an account, returns a session token
POST /api/auth/signin        -> returns a new session token
POST /api/auth/forgot-password -> emails a one-time reset link (generic reply)
POST /api/auth/reset-password -> sets the new password, revokes all sessions
POST /api/auth/signout       -> revokes the bearer session
GET  /api/auth/me            -> the signed-in account
POST /api/payments           -> validates a request, returns the QR payload
GET  /api/payments/{pda}     -> live status read from the cluster
POST /api/payments/watch     -> status of many payment addresses at once
GET  /api/solana-pay/{token} -> wallet metadata for a scanned QR (label/icon/message)
POST /api/solana-pay/{token} -> unsigned settle_payment transaction for the wallet
GET  /api/solana-pay/icon.svg -> the icon wallets display beside that label
GET  /api/discounts          -> the signed-in account's discount codes
POST /api/discounts          -> creates a code {code, percent_off}
POST /api/discounts/validate -> prices an invoice with a code
GET  /api/admin/stats        -> account/session/code/reset counts (admin)
GET  /api/admin/users        -> one page of accounts; ?search=&limit=&offset= (admin)
DELETE /api/admin/users/{id} -> removes an account and everything it owns (admin)
GET  /api/admin/payments     -> recent on-chain invoices, newest first (admin)
DELETE /api/discounts/{id}   -> removes a code
```

### Accounts

Passwords are hashed with Argon2 and sessions are opaque random tokens. The
client keeps the token in `localStorage` and sends it as
`Authorization: Bearer <token>`; the server stores only the SHA-256 digest of
each token, so a leaked database dump cannot be replayed as a live session.
Sessions last 30 days.

Sign-up is deliberately instant — no email confirmation stands between a new
visitor and the app. Email only appears in **password recovery**:
`POST /api/auth/forgot-password` answers the same generic message whether or
not the address has an account (no account enumeration) and emails a
one-hour, single-use link. `POST /api/auth/reset-password` swaps the
password, consumes the link, and revokes every session of the account —
old sessions die with the old password. Locally the API has no SMTP
configured, so the reset link is printed to its console (the equivalent of
Django's console backend); with `EMAIL_HOST` set it is delivered over SMTP —
Gmail SMTP with an app password is the free production setup.

### Admin

The admin panel lives at `/admin` in the web app and is backed by three
`/api/admin/*` endpoints. Access is **configuration, not schema**: the API
matches the signed-in account's address against `ADMIN_EMAILS` (comma
separated), so granting or revoking access never needs a migration — add or
remove an address and redeploy. Every admin endpoint re-checks the session
first (401 when signed out) and then the address (403 otherwise). The panel
shows platform counts and the account list with live session and discount
code counts, and can delete an account along with its sessions, codes and
pending reset links; deleting your own account is refused.

`POST /api/payments` requires a valid session. `GET /api/payments/{pda}` is
public on purpose — a scanner must be able to read an invoice before deciding
whether to pay, and the endpoint only ever reads from the cluster.

The pay page itself needs no account: anyone holding a payment link can open
it, connect a wallet and pay, and `POST /api/discounts/validate` is public for
the same reason. Only creating invoices, managing discount codes and the
monthly budget require a session.

### Discount codes

Codes are per account, not per invoice: a seller creates percentage-off codes
on the Sell screen and the same list works for every QR they create. The buyer
types a code into the scan-to-pay screen, `POST /api/discounts/validate`
prices it (public, like the status read), and the screen shows the discounted
total before the pay button.

The amount stays bound into the payment address, so a discounted payment
derives its *own* address rather than changing the invoice: the seller's
screen watches the invoice address plus one address per code (`POST
/api/payments/watch`, one RPC round trip) and reports which code was used by
whichever address gets funded. A buyer can therefore never pay less at the
invoice address itself — underpayment shows up as a different, watched
address.

### Checks

```bash
pnpm lint
pnpm typecheck
cargo test --manifest-path apps/api/Cargo.toml
cargo test --manifest-path apps/contracts/Cargo.toml   # Anchor workspace
```

## Environment variables

Local values live in `apps/web/.env` (copy `apps/web/.env.example`); Nuxt
loads it before evaluating `nuxt.config.ts`, and the API loads that same file
on startup.

| Variable                        | Where        | Default         | Purpose                                   |
| ------------------------------- | ------------ | --------------- | ----------------------------------------- |
| `SOLANA_CLUSTER`                | web + api    | `devnet`        | Cluster the app targets (localnet/devnet/testnet/mainnet) |
| `NUXT_PUBLIC_API_BASE`          | web          | `""` (same origin) | Origin of the Axum API in production   |
| `NUXT_PUBLIC_SOLANA_ENDPOINT`   | web          | cluster default | Custom RPC endpoint                       |
| `BIND_ADDR`                     | api          | `127.0.0.1:8787`| Local listen address                      |
| `SOLANA_RPC_URL`                | api          | cluster default | RPC endpoint the API uses for status reads |
| `DATABASE_URL`                  | api          | `data/payvand.db` | libSQL server URL; unset means embedded SQLite |
| `LIBSQL_AUTH_TOKEN`             | api          | —               | Required when `DATABASE_URL` is remote   |
| `FRONTEND_BASE_URL`             | api          | `http://localhost:3000` | Origin that password-reset links point at |
| `EMAIL_HOST`                    | api          | — (console)     | SMTP host; unset prints the email to the API console |
| `EMAIL_PORT`                    | api          | `587`           | SMTP port                               |
| `EMAIL_HOST_USER` / `EMAIL_HOST_PASSWORD` | api | —            | SMTP credentials — a free Gmail app password works |
| `EMAIL_USE_TLS`                 | api          | `true`          | STARTTLS on the SMTP connection          |
| `DEFAULT_FROM_EMAIL`            | api          | SMTP user       | Sender address                           |
| `ADMIN_EMAILS`                  | api          | — (nobody)      | Comma-separated addresses allowed into `/api/admin/*` |

Never put a wallet secret key or a seed phrase anywhere in this repository. The
Nuxt module deliberately strips `payer` and `payerSecretKey` from its public
runtime config for the same reason.

## Deploying to Vercel (Hobby plan)

Vercel cannot host both parts in one project, so the repository deploys as **two
Vercel projects** pointing at the same repo. Both work on the Hobby plan.

### 1. Web app

- Import the repo in Vercel with **Root Directory = `apps/web`**.
  Vercel detects Nuxt automatically (`apps/web/vercel.json` pins the
  framework). CLI deploys (`vercel deploy --prod`) are run from inside
  `apps/web` so the upload tree is the app itself.
- Set `NUXT_PUBLIC_API_BASE` to the origin of the API project below.

### 2. API

- Create a second Vercel project from the same repo with
  **Root Directory = `apps/api`** and Framework Preset = **Other**. CLI
  deploys are run from inside `apps/api`.
- `apps/api/vercel.json` rewrites every path to the Rust binary
  (`src/main.rs`, the `@vercel/rust` zero-config entrypoint), which mounts the
  same Axum router through `vercel_runtime`'s `VercelLayer`. The local dev
  server is the separate `src/bin/server.rs` bin (`pnpm api`). The Rust
  runtime is currently in beta on all plans.
- Redeploy the web project after the API has a URL so it picks up
  `NUXT_PUBLIC_API_BASE`.

The API only stores accounts, sessions and nothing else — payment records are
still derived from the request and the chain, so separate serverless isolates
never disagree. Point `DATABASE_URL` at a managed libSQL server (Turso or
compatible) in production; an embedded SQLite file does not survive a
serverless cold start.

## Notes and next steps

- The QR encodes a [Solana Pay transaction request](https://docs.solanapay.com/spec):
  `solana:<api-origin>/api/solana-pay/<token>`, where the token is the same
  invoice JSON (`{v,id,seller,seller_user,reference,amount_lamports,memo,cluster,endpoint}`)
  base64url-encoded. A wallet recognizes the scheme, POSTs its account and
  signs the `settle_payment` transaction the API builds, so it settles exactly
  like a payment made inside the app. The app's scanner decodes the token back
  into the invoice, and a pasted raw payload or `?p=` share link still works.
  The shareable link (`<origin>/pay?p=<payload>`) is unchanged and remains the
  way any phone camera opens the printable payment page.
- Cluster defaults in the API are the public `api.*.solana.com` endpoints. For
  anything beyond testing, set `SOLANA_RPC_URL` to a dedicated provider.
- `settle_payment` creates the invoice account (funded by the buyer) and moves
  lamports straight from the payer to the seller in one transaction. The amount
  is part of the PDA seeds, so a code can only ever be settled for exactly the
  amount it was shown with. `close_payment` lets the seller reclaim the rent
  once it has been settled.
