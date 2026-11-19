# 0005 — Per-shop fee overrides

Status: `Draft`. Follow-up to `0002`, which keeps one global policy (`Config`)
and lists "per-shop fee overrides" as out of scope and as an open question. This
file is where the override is designed; it is not approved for implementation.

## What exists today (`0002`, revision 17)

- One policy for the whole program: `Config { fee_bps, fixed_fee_lamports,
  fee_mode, session_fee_float, version }`, read by all four settle paths.
  `helpers::fee_for(config, amount)` computes
  `min(amount * fee_bps / 10_000 + fixed_fee_lamports, amount - 1)` and is the
  single place the split is decided.
- A **shop is an API row, not an account**: `shops(id, user_id, name,
  description, created_at)` plus `products`, in `apps/api/src/db.rs`. A seller
  may keep several shops (a café and a bakery), and the invoice's lines come
  from the shop that priced it.
- An **invoice carries no shop**: `Payment`/`TokenPayment` hold seller,
  reference, amount, memo, mint; their PDAs are seeded by
  `[payment, seller, reference, amount]` (plus mint for tokens). The shop exists
  only in the API's copy of the receipt.
- `PaymentSettled` carries `amount`, `fee`, `seller_proceeds`, `payer`,
  `seller`, `mint`, `reference` — enough to price a settlement only if the fee
  is the one `Config` held at that slot.

## Why it is not a column on `shops`

1. **The program cannot read SQLite.** A shop-level rate has to be enforceable
   on chain, or a seller can edit the API's row and keep the difference.
2. **The seller is the on-chain identity, and it owns several shops.** A
   seller-keyed policy would apply one rate to every shop the wallet runs, which
   is not "per shop". A shop-keyed policy needs an on-chain shop.
3. **Invoice seeds cannot grow for free.** Adding a shop to the seeds changes
   every QR a seller has already printed, and settle creates the invoice
   account, so the shop must arrive as an **account**, not as invoice data.
4. **The event becomes the only truthful price.** If two shops charge different
   rates, an indexer can no longer recompute `fee` from `Config` at the settle
   slot; the event (or a per-receipt policy field) is what makes the split
   auditable.
5. **The existing `version` field is the migration hook.** `Config` already
   carries one and `0003` plans to bump it for a mint policy; whichever lands
   first owns the bump.

## Tasks

### Task A: Decide the shape before the code

- [ ] Decide what a "shop" is on chain: a `ShopPolicy` PDA
      (`[shop-policy, seller, shop_key]`) created by the seller and confirmed by
      the admin, a seller-level `SellerPolicy` that shops share, or no policy
      account at all and an off-chain-only rate with the global fee unchanged.
      Record the decision here.
- [ ] Decide who may set a rate and within what band: admin-only, seller-set
      within `Config`'s range, or seller-set below a hard cap. Decide what
      happens to a shop whose rate is out of band at settle time.
- [ ] Decide the fallback: a settlement with no policy account charges
      `Config`'s rate, so every invoice printed before this lands still settles.
- [ ] If a shop policy is chosen: decide its identity — the API's 16-byte hex id
      is not an address, so it needs either a hash in the seeds or a freshly
      generated keypair whose address the API stores beside the row.

### Task B: Program — policy accounts and `fee_for`

- [ ] Add the policy account with `InitSpace`/space constant, its layout
      assertion beside `account_spaces_match_the_sizes_used_by_the_program`,
      and a named error for a policy that does not belong to the invoice's
      seller.
- [ ] Split `fee_for` into `fee_for(policy, amount)` with the policy resolved
      once per settle: the shop policy when present, `Config` otherwise. All
      four settle instructions use it — including `pay_from_budget` and
      `pay_from_token_budget`, where the invoice names the seller and the
      policy therefore travels with the invoice.
- [ ] Keep the split arithmetic cap (`amount - 1`) and the
      `InvoiceTooSmall` floor identical, so a rate change cannot create an
      underflow the old one could not.
- [ ] Record the policy on the settlement: a field on `PaymentSettled` (and,
      if the receipt is to show it, on `PaymentReceipt` with the space bump) so
      a watcher does not have to guess which rate applied.

### Task C: Admin and configuration

- [ ] `initialize_config`/`update_config` keep their shape; the band and the
      default live in `Config` (a `min_fee_bps`/`max_fee_bps`, or one cap) and
      are validated on both set and settle.
- [ ] An admin path to approve, cap or revoke a shop policy, plus the event that
      records it.

### Task D: API and client

- [ ] Store the policy address (and the rate, for display) beside the shop row;
      migrate existing rows to "no policy, `Config`'s rate".
- [ ] Build the invoice with the policy account when the shop has one — the
      checkout and the seller's QR both — and price the quote from the same
      source of truth the program reads, never from a cached copy.
- [ ] Show the seller the effective rate per shop and warn when a policy is
      missing or stale; show the buyer only the price, as today.
- [ ] Check `apps/api`'s receipt building and admin feed, which currently derive
      the fee from `Config`; they must read the event's `fee` instead.

### Task E: Tests and devnet

- [ ] Harness: two shops under one seller with different rates settle for
      `amount - fee_shop` each, with the treasury receiving each shop's fee.
- [ ] Harness: an invoice with no policy charges `Config`'s rate (the fallback),
      and a policy whose seller differs from the invoice's is refused by name.
- [ ] Harness: a policy at the cap leaves the seller one base unit, as
      `a_ten_thousand_bps_policy_leaves_the_token_seller_one_base_unit` does for
      the global rate.
- [ ] Client: quote tests for a shop with and without a policy, and one where
      the policy is out of band.
- [ ] Devnet: set a shop rate, settle one invoice through it, and record the
      signature and the split here.

## Acceptance criteria

- [ ] A settlement's fee is the rate of the shop that priced it, and the event
      and receipt agree with the lamports that moved.
- [ ] An invoice with no policy, or one printed before this ships, settles at
      `Config`'s rate unchanged.
- [ ] No seller can raise its own rate above the configured band, and no
      settlement can name another seller's policy.
- [ ] `README.md` states that the fee is per shop, with a pointer to the band.

## Verification

- [ ] `cargo build-sbf --manifest-path programs/payvand/Cargo.toml`, then
      `cargo test --test settle` in `apps/contracts` (never `anchor test` here).
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
- [ ] `anchor idl build`, copied to `apps/web/app/assets/payvand.idl.json`.
- [ ] `apps/api` tests for the shop row migration and the fee it reports.
- [ ] In `apps/web`: `vitest run`, `nuxt typecheck`, `eslint .`.
- [ ] Devnet smoke run recorded here.

## Dependencies

`0002` Task 4.3 (the `PaymentSettled` split the event must keep truthful) and
Task 1.6 (the fee boundary tests that must hold at any rate). `0003` also plans
a `Config` layout bump for a mint policy — whichever lands first takes the
version bump, and the second must build on it rather than assume the original
layout.

## Risks And Mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Quote and settle disagree on the rate | High | One source of truth per shop; the API reads the policy, the program enforces it, and the event reports what happened. |
| A seller sets a rate that hides its own fee | Medium | Band enforced on chain at set time and settle time, not only in the UI. |
| Old QRs stop working | High | The account is optional and the fallback is `Config`'s rate, proven by a test. |
| Indexers keep pricing from `Config` | Medium | The fee and the policy address ride in the event; `apps/api` is migrated in the same change. |
| Two plans bump `Config` at once | Medium | One bump, sequenced with `0003`, recorded in `DEV.md`. |

## Rollout

- [ ] Program: upgrade in place, IDL copied, `Config` migrated or version-bumped
      once.
- [ ] API: shop rows gain a nullable policy address; the admin feed reads the
      event's fee.
- [ ] Client: quotes and builders pass the policy when it exists.
- [ ] Smoke tests: one shop-rate settlement and one fallback settlement on
      devnet.

## Open Questions

- Does the product want a marketplace take per shop, or per seller with shops
  inheriting it? The on-chain shape differs, and only the second is one account
  per wallet.
- Should a shop be able to set **zero** fee (a free shop), or is the band's
  floor above zero to keep the treasury funded?
- Who approves a rate change — the admin, or the seller within the band?
- Do discounts (`discount_codes`) and shop rates interact, and in what order do
  they apply to the price shown?
