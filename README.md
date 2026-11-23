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
| Solana    | Anchor 1.2.1 program (`apps/contracts/programs/payvand`)   |
| Hosting   | Vercel (web project + API project)                         |

[vs]: https://vue-solana-docs.vercel.app/packages/nuxt

## Repository layout

```
apps/web/            Nuxt app (run `pnpm dev` from the repo root)
  app/pages/sell.vue     Seller: create invoice + render QR (requires an account)
  app/pages/pay.vue      Buyer: the shared, printable payment page (name, price, QR)
  app/pages/scan.vue     Buyer: scan a QR in the app and pay from this device
  app/pages/budget.vue   Buyer: a monthly allowance a session key spends from
  app/pages/(auth)/signin.vue   Email + password sign-in
  app/pages/(auth)/signup.vue   Create an account
  app/assets/payvand.idl.json   Copy of the Anchor IDL used by the client
apps/api/            Axum API (also the Vercel Rust function)
apps/contracts/      Anchor program + workspace Cargo.toml/Anchor.toml
  programs/payvand/      The on-chain program
  target/                Anchor build output — includes the program keypair
plans/               Implementation plans (numbered; templates + archive/)
Taskfile.yml         Task runner entry points (`task --list`)
```

The web app is a standalone pnpm project in `apps/web` (its own lockfile);
the repo-root `package.json` only forwards the documented commands into it.

## Prerequisites

- Node 20+ and pnpm
- Rust (stable) and Cargo
- [Solana CLI][solana] and [Anchor CLI 1.2.1][anchor]
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

### Publishing the IDL

The on-chain IDL is not part of the program binary: it lives in a Program
Metadata account (seed `idl`), so `pnpm anchor:deploy` leaves it describing the
previous interface. Publish a changed one with `task idl:upload` (or
`pnpm anchor:idl` without `task`) — it builds the IDL, uploads it, then fetches
it back and compares — or run the same steps by hand from `apps/contracts`:

```bash
anchor idl build -o target/idl/payvand.json
npx @solana-program/program-metadata \
  --rpc https://api.devnet.solana.com -k ~/.config/solana/id.json \
  write idl 4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J target/idl/payvand.json
```

`[Warning] RPC rate limited (HTTP 429), retrying…` lines are normal on the
public devnet RPC; the command retries them and finishes.

Confirm the cluster serves what you built (the fetch itself can need a retry
while the RPC is busy):

```bash
anchor idl fetch 4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J -o /tmp/live.json
diff <(python3 -m json.tool /tmp/live.json) <(python3 -m json.tool target/idl/payvand.json)
```

#### Why `anchor idl init` / `upgrade` are not the route

Anchor 1.2.1 talks to the same Program Metadata program, but its IDL commands
build one multi-transaction plan and abort it when the public devnet RPC rate
limits a later send: `create-buffer` lands the buffer's own transaction, then
dies with `HTTP error (429): Too Many Requests` — or with the bare `The provided
transaction plan failed to execute` when only the plan wrapper's message survives
— and exits 1. `init` and `upgrade` wrap that same plan, so they abort before
anything is written and the previous IDL stays live. This is **not an on-chain
refusal**: the Anchor CLI does not retry a 429, while the npm CLI retries the
identical create/write/set sequence on the same wallet and cluster and finishes.

`anchor idl create-buffer` followed by `anchor idl write-buffer <program>
--buffer <address> --close-buffer` is **not** a safe workaround. `write-buffer`
reports `Metadata written successfully` against the buffer the aborted
`create-buffer` left behind, but the account ends up holding a truncated
compressed payload — its length field sits at the full 10,112-byte capacity with
an incomplete zlib stream — and every reader then fails with
`[Error] undefined` until a real upload repairs it. That was observed on devnet
on 2026-11-21; re-publishing with the npm command above restored the account.

A buffer an interrupted run leaves behind holds ~0.05 SOL of rent;
`npx @solana-program/program-metadata list-buffers` and
`close-buffer <address>` recover it.

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
GET  /api/faucet             -> whether the signed-in account can claim starter tokens
POST /api/faucet             -> sends them to the account's linked wallet (once per account)
```

### Starter tokens

`/start` is the way in for someone who has never held a wallet: it links to
Solflare's download page, explains devnet, shows how to switch networks in
Solflare, Phantom or Backpack, and links the learning guides under `/learn`.

On that page (and on the profile) a signed-in account can claim a starting
balance — 1 SOL, 10 USDC and 10 EURC by default — sent from the API's own
treasury to the wallet the account linked. The claim is recorded in
`faucet_claims` before anything is signed, so an account can claim once and a
double tap cannot send twice; a failed transfer releases the claim. The
amounts and the treasury come from `FAUCET_KEYPAIR`,
`FAUCET_SOL_LAMPORTS` and `FAUCET_TOKEN_BASE_UNITS`, and nothing is sent when
the treasury is unset or the cluster is not a test network. The treasury must
already hold devnet SOL and the USDC/EURC mints' tokens — it cannot mint them.
Spent tokens can be topped up at `faucet.solana.com` and `faucet.circle.com`.

The devnet treasury the faucet pays from is
`H4ZxTWkjjgCvuJEcAEv48wJipsE62GJ4WwTBsxiXDj9o` — its secret lives outside the
repository (Vercel's `FAUCET_KEYPAIR` secret) and it needs its own devnet SOL,
USDC and EURC before a claim can be sent.

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

### Shops subscription

Shops and products are sold by the month: **€1.70 for 30 days**, payable in
SOL, USDC or EURC. The price is a euro price, so EURC pays at par while USDC and
SOL are converted at a live rate (`SUBSCRIPTION_RATES_URL`, cached for five
minutes by the API) and rounded up to the base unit. With no URL configured the
API reads the same keyless Coinbase feed the payment page's fiat hints come
from, so the plan's quote and the prices beside a checkout agree; CoinGecko's
simple-price feed is the fallback behind it, and the quote is refused only when
both are down. `GET /api/subscription` returns the account's period and, when a
rate could be read, all three amounts — an outage leaves the period and plan
intact with `rates_available: false` and no quotes, so the card still shows
where the subscription stands while the currency buttons wait.

Paying is an ordinary invoice from the platform wallet (`PLATFORM_SELLER`):
`POST /api/subscription/invoice` quotes the currency, records the invoice for
that account, and returns the same code shape a seller's QR carries, so the
checkout, the wallet and the program's settle paths are unchanged. Coming back
from the checkout, `POST /api/subscription/confirm` asks the cluster whether
that invoice settled — nothing the browser says is trusted — and only then
records the period, adding 30 days to whatever is left. Nothing about the plan
changes the percentage fee on ordinary invoices.

A lapsed subscription does not hide existing shops or break printed codes: it
refuses creating or editing shops and products (403), which is what the card on
`/shops` is there to settle.

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

### Fees, rent and the treasury

The fee policy is on chain in `Config`, not in the app: every settlement reads
it, so changing the rates takes effect without a release. The default is `100`
bps up to 0.1 SOL (or 10 token units) and `50` bps above it, `SellerPays` — the
buyer is charged the price the QR shows, never the price plus something — and a
permanent 48-byte receipt for every settled invoice.

**A shop can cut the rate, never raise it.** `set_shop_policy` writes a
`ShopPolicy` PDA (`["shop-policy", seller, shop_key]`, 75 bytes) holding one
multiplier in basis points on `Config`'s two rates, signed by the shop's own
seller. The band is the write: `0..=10_000`, so a shop can discount itself to
nothing — a token invoice still pays the receipt's flat, so a free shop cannot
put the record on the treasury — and `ShopFeeTooHigh` refuses anything above
`10_000`, which is `Config` unchanged. The settle paths take that account as an
optional trailing account; an invoice that names none, including every QR
printed before this existed, pays `Config`'s rate exactly as it did. A policy
recorded under another seller is refused with `ShopPolicySellerMismatch`, so one
shop cannot price another's invoice.

**The receipt's rent is the floor of the fee.** A token seller cannot be charged
lamports, so on that path the treasury funds the receipt and collects it back as
a flat in the currency (`flat_usdc_base_units`, `flat_eurc_base_units`); the fee
is `max(rate, flat)`. A SOL seller hands the rent over out of the price directly,
so the rate has it taken off instead: `max(0, rate − receipt rent)`. Either way
the seller's total is `max(rate, receipt rent)` — a payment too small for the
rate to cover the record is charged the record and nothing else, so the
treasury neither takes a fee on it nor pays for it.

| Who | Pays | Ends up with |
| --- | --- | --- |
| Buyer | the price, plus the Solana network fee (only the price, when a sponsor pays it) | out exactly `amount` plus that fee, and the receipt on chain |
| Seller | the larger of the rate and the receipt's rent, out of the price | `amount − max(rate, receipt rent)` |
| Treasury | the receipt's rent on the token path, nothing on SOL | `max(rate, receipt rent) − receipt rent` |

How a SOL settlement gets there, in one signature: the payer funds the invoice
account and the receipt; `amount − invoice rent − receipt rent − fee` goes to the
seller; `fee` goes to the treasury; the invoice account closes to the seller with
its rent; and the treasury pays the payer back the receipt rent. The payer's
balance therefore moves by exactly `amount` plus the network fee, and the only
lasting cost is the receipt, which the seller paid for and which nobody can
close.

Token invoices take the fee in the token: `amount − fee_t` to the seller and
`fee_t` to the treasury's own account for that mint. **Sellers need no SOL** to
be paid in a token, and the payer's SOL exposure is the network fee alone: the
seller's token account must already exist, on both the wallet and the allowance
path, because its rent would be a cost nobody refunds and the buyer's money
would be paying it. The treasury must
have its account for a mint opened before an invoice in that mint can settle —
`pnpm treasury:open --mint <mint>`; the checkout reads the account and refuses
to let anyone sign before it exists.

That network fee is the one thing a wallet must hold SOL for, and on a token
invoice it can be someone else's. `settle_token_payment` takes a `sponsor`: it
advances the invoice's rent and the receipt's and receives the treasury's refund
of both, so its net cost is the network fee it signs for, and the buyer's wallet
ends where it began. A wallet holding SOL passes itself as its own sponsor — one
signature, no change in cost — and a wallet holding only the token passes a
relayer's key instead, which is what makes a USDC-only buyer able to pay. The
refund goes to the sponsor and never to the buyer on purpose: a sponsor that
funded the buyer's wallet would leave it holding both rents, 3,939,360 lamports
per settlement at mainnet's rate, which is a faucet. A SOL invoice cannot be
sponsored this way — the buyer's own lamports are the payment.

A budget payment is the same ledger with the pot in place of the wallet: the pot
pays exactly the price, and a float fronts the temporary rents and is reimbursed
inside the same transaction. In SOL the session key carries the whole 30,000,000
lamports (`Config::session_fee_float`). In a token the budget account holds its
own rent and nothing more: the session key fronts the invoice and receipt rents
and the fee out of `SESSION_FEE_FLOAT` (12,000,000), and the treasury pool
refunds the two records in the same transaction, so a payment costs the float
its signature and the peak it has to front is 3,939,360 lamports plus that fee.
The seller's token account is not part of it: a seller who has not opened one is
refused, exactly as on the wallet path, and the buyer tops the key up only when
it is short of that peak.

**Break-even.** With the rent as the floor, no settlement loses money: the
treasury's delta is `max(rate, receipt rent) − receipt rent` on the token path,
which is never negative, and `max(0, rate − receipt rent)` on SOL, where it pays
nothing. What a small invoice earns is nothing rather than a subsidy, and the
first fee-sized margin appears once the rate overtakes the record.

**Planned compressed receipts (`plans/0006`).** When compressed receipts are
enabled, a settlement commits the invoice to one of four shared Merkle trees
instead of opening a permanent receipt account. The same `100`/`50` bps tiers
apply, but there is no receipt-rent floor, no rent-derived token flat, and no
receipt-rent refund; only the invoice-rent refund remains on token payments.
Tree, nullifier, and occupancy accounts are shared infrastructure paid for by
the operator and the treasury, old trees remain verifiable after rotation, and
the legacy marker path stays available for rollback. Rotation is not free: a
new generation costs **3.900 SOL** of permanently locked rent, of which the
operator keypair that creates and prepares the four new trees pays **1.234 SOL**
and the treasury pays **2.666 SOL** for the four occupancy bitmaps. The
treasury's share is held back rather than merely recorded: while compressed
receipts are enabled, `withdraw_treasury` reserves the next token settlement's
refund plus the four occupancy bitmaps, so fees cannot be withdrawn out of the
rent the next rotation needs, and settlement refunds are the one remaining drain
the balance alert watches. The treasury is funded for that share before the
first rotation, and a treasury short anyway refuses the rotation instead of
half-applying it. Until that plan ships, the accounting above is the live
behaviour and the reserve stays at one settlement's refund.

**Keeping the flats live.** The program has no oracle, so the two token flats
are written from a price rather than computed at settle time:
`refresh_fee_price(sol_usd_micros, sol_eur_micros)` recomputes both from the
cluster's current `Rent::minimum_balance(PAYMENT_RECEIPT_SPACE)` and the price it
is given. It is permissionless — `pnpm fee:refresh --cluster devnet` fetches the
rate and sends it — and bounded by the program: a price must be plausible, it
must sit within `PRICE_ANCHOR_BAND_BPS` (10%) of the price the admin anchored
with `set_fee_price_anchor` (`pnpm fee:anchor`), and a second call inside
`FLATS_REFRESH_SECS` (60 s) is refused. The band is measured from that anchor,
never from the flat the last refresh wrote, so repeated calls cannot walk the
flats anywhere: however many run, every price they accept is in the same window.
Run it on the same cadence as the app's rate cache, or whenever SOL has moved;
use `pnpm fee:anchor` when the price should move further than the band in one
step. A settlement is refused with
`TreasuryUnderfunded` before it moves anything when the treasury cannot cover the
refund it owes, so a drained treasury blocks payments rather than losing them.

Rent is read from the cluster (`Rent::minimum_balance`), never from a constant,
so these figures follow it. The addresses, the current `Config` values, and the
commands that change the policy or top the treasury up are in `DEV.md`.

### Token programs

Payvand settles **SPL Token mints only**. Every token context checks that the
mint is owned by the SPL Token program and carries the 82-byte SPL mint layout
before it moves anything, so a Token-2022 mint is refused at the account check
rather than failing somewhere inside a transfer. The refusal has a name:
`PaymentError::Token2022Unsupported`, `Custom(6017)` — "Payvand settles only SPL
Token mints; Token-2022 is not supported" — which is what a Token-2022 settle
returns. Owner and layout are the whole currency rule: the mints the app offers
(SOL, USDC and EURC in `CURRENCY_MINTS`) are SPL Token mints on both clusters,
and the treasury needs its own account per mint (`pnpm treasury:open --mint
<mint>`) before an invoice in that mint can settle.

The token accounts and the token program are typed with `anchor-spl`'s interface
types (`InterfaceAccount<TokenAccount>`, `Interface<TokenInterface>`), which
parse either program, so the mint's owner is what does the refusing — the error
names the mint instead of surfacing Anchor's generic
`AccountOwnedByWrongProgram` (3007). The transfers themselves are still SPL
Token's.

Supporting Token-2022 — interface CPIs, `transfer_checked_with_fee`, the rules
for extensions such as transfer fees and permanent delegates, ATA derivation per
token program, and a treasury account per mint *and* program — is a follow-up of
its own: [plans/0003-token-interface-migration.md](plans/0003-token-interface-migration.md).
Until it lands, no Token-2022 invoice can be paid: the app offers no such mint,
and the program would refuse one.

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
| `PLATFORM_SELLER`               | api          | —               | Wallet that sells the shops subscription; unset disables buying one |
| `SUBSCRIPTION_RATES_URL`        | api          | Coinbase        | EUR/SOL and EUR/USD price feed the subscription quotes use; CoinGecko is the built-in fallback, and setting this URL replaces both |
| `FAUCET_KEYPAIR`                | api          | —               | Treasury that sends the devnet starter tokens; a solana CLI `id.json` array or base58. Unset disables claiming |
| `FAUCET_SOL_LAMPORTS`           | api          | `1000000000`    | Lamports of SOL a claim sends (1 SOL)      |
| `FAUCET_TOKEN_BASE_UNITS`       | api          | `10000000`      | Base units of USDC and EURC a claim sends (10 tokens) |

Never put a wallet secret key or a seed phrase anywhere in this repository. The
Nuxt module deliberately strips `payer` and `payerSecretKey` from its public
runtime config for the same reason.

## Deploying to Vercel (Hobby plan)

Vercel cannot host both parts in one project, so the repository deploys as **two
Vercel projects** pointing at the same repo. Both work on the Hobby plan.

### 1. Web app

- Import the repo in Vercel with **Root Directory = `apps/web`**.
  Vercel detects Nuxt automatically (`apps/web/vercel.json` pins the
  framework).
- Set `NUXT_PUBLIC_API_BASE` to the origin of the API project below.
- A CLI deploy uploads the **project root tree**, not the app folder: with
  Root Directory set to `apps/web`, run `vercel deploy --prod` from a tree that
  contains `apps/web`. Uploading only the app folder builds fine until
  `nuxt.config.ts` reads the workspace `package.json` for the footer version
  (`../../package.json`), so include that file too. The repository root itself
  can exceed Vercel's 10 MB upload limit; a two-file tree
  (`package.json` + `apps/web/`) deploys the same app. `task deploy:web` builds
  exactly that tree, deploys it and removes it again.

### 2. API

- Create a second Vercel project from the same repo with
  **Root Directory = `apps/api`** and Framework Preset = **Other**. CLI deploys
  follow the same rule: this project's Root Directory is `.`, so `apps/api`
  itself is the upload tree (no `target/`, `.vercel` or `.env*`);
  `task deploy:api` assembles and deploys it.
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
- `settle_payment` creates the invoice account (funded by the payer), moves
  `amount − fee` to the seller and `fee` to the treasury, writes a permanent
  receipt at `["payment-receipt", invoice]` and closes the invoice account in
  the same transaction, so its rent returns to the seller without a second
  signature — while the treasury refunds the receipt rent so the payer is
  charged exactly the price. The amount is part of the PDA seeds, so a code can
  only ever be settled for exactly the amount it was shown with, and the
  receipt's `settled_at` makes a second settlement fail with `AlreadySettled`.
  There is no close or reclaim instruction left to call.
