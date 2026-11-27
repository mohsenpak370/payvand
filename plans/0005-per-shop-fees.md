> **Deprecated / not needed:** This feature (per-shop fee multiplier) is not exposed in the webapp and the fee is deducted from the seller's side, so a shop lowering its rate only benefits the seller and costs the protocol. It has been removed from user-facing documentation (FAQ). Retain for historical context only.

# 0005 — Per-shop fee overrides

Status: `In Progress` (2026-11-22). Follow-up to `0002`, which keeps one global
policy (`Config`) and lists "per-shop fee overrides" as out of scope and as an
open question. This file is where the override is designed.

Increment 1 (2026-11-22): Task A decided and recorded below, and the fee
arithmetic takes a shop multiplier — `helpers::fee_for` scales the rate by it,
and all four settle paths pass `SHOP_FEE_BASE_BPS` (10,000) so nothing a caller
can observe has changed yet. `ShopPolicy`, its seed and its space constant are
in `state`/`constants`, and a unit test pins the arithmetic: half is half of the
percentage, zero still pays the marker's flat on a token invoice, and the
boundaries, the flats and the `amount - 1` cap are untouched. Verified:
`cargo build-sbf` clean, `cargo test --test settle` 60 passed (59 before plus the
multiplier test), `cargo fmt --check` clean. The IDL is unchanged — a type no
instruction names is not emitted — so nothing was copied to `apps/web`.

Increment 2 (2026-11-22): the setter and the optional account shipped.
`instructions/shop.rs` holds `set_shop_policy` — seller-signed, `init_if_needed`
so a rate change is one signature, the `0..=10_000` cap enforced on write — with
`PaymentError::ShopFeeTooHigh` (6021) above it and
`PaymentError::ShopPolicySellerMismatch` (6022) for a policy recorded under
another seller, which `helpers::shop_multiplier` applies on every path. The four
settle contexts take `shop_policy: Option<Account<ShopPolicy>>` as a **trailing**
account, seeded by the policy's own seller and shop key, so a stand-in is a seeds
failure and a foreign seller is the named error. `allow-missing-optionals` is now
on for `anchor-lang`: without it Anchor demands a slot for a trailing optional
account (the program id stands in for "none"), so every settle built before this
would have failed `AccountNotEnoughKeys` instead of paying `Config`'s rate.
Verified: `cargo build-sbf` clean, `cargo test --test settle` 65 passed (60
before, plus the layout pin, the setter/cap/seeds test, the SOL shop-rate test,
the two-shops test and the foreign-seller test), `cargo fmt --check` clean,
`cargo clippy --all-targets` at its three pre-existing warnings and nothing new.
*(Historical count: the 2026-11-26 audit found six test lints — see `0003`.)*
The IDL was rebuilt and copied to `apps/web/app/assets/payvand.idl.json`, and the
web builder now honours the IDL's `optional` flag by ending the account list there
— without that, every settle the app builds would ask for `shop_policy` by name.

Increment 3 (2026-11-22): the allowance paths are covered and the relayer can
rebuild a shop invoice. The harness now exercises all four settle paths at a
shop's rate — the two wallet paths in increment 2 and
`the_budget_path_charges_the_shop_rate` / `the_token_allowance_charges_the_shop_rate`
here, each asserting the seller's share and the treasury's, and that the pot pays
exactly the price whatever the split. `apps/api/src/sponsor.rs` takes an optional
`shop_policy` in its request and appends it as the trailing read-only account the
program declares, so a sponsored settle of a shop invoice can match the buyer's
message; without it the rebuild was 14 accounts and every such settle would have
been refused. Verified: `cargo build-sbf` clean, `cargo test --test settle` 67
passed, `cargo fmt --check` clean; `apps/api` 87 tests passed (`cargo fmt
--check` clean); web `nuxt typecheck` 0 errors and `eslint` clean on the touched
composable, whose `SponsoredSettleInput` gained the optional `shopPolicy`. The
IDL is unchanged. Nothing passes a policy yet: the checkout has no address to
name until Task D stores one.

Next: the `PaymentSettled` field, then the API/client side (Task D) — storing the
policy address beside the shop row and passing it to the quote, the builder and
the sponsor request.

Progress audited 2026-11-26 against `92010cb`: **still In Progress; the program
and relayer halves are shipped and unchanged by the later refactors.** All four
settle paths still resolve `ShopPolicy` and all four harness tests remain; the
relayer still accepts a trailing `shop_policy`. The current settle suite is 74
tests (this plan records 57/60/65/67 at earlier increments); after a fresh
`cargo build-sbf` it passes 74/74 and `cargo fmt --check` is clean. Clippy is
still not clean — six test lints now, listed in `0003`'s header. The `Config`
below was v3 when drafted but is **v4** today; `0007` took v4 for the network-fee
fields, and this plan's multiplier shape needs no `Config` byte at all.
`shops.currency` now exists (shop-menu pricing), but no shop row stores a policy
address yet. **What is left:** the event field, the shop-row column, the
API/client wiring (quote, checkout, allowance and seller UI), a settlement
through a policy written at `10_000`, client quote tests and a devnet record.
The `Config` band and admin approve/revoke boxes are decided away, not overdue.

## What exists today (re-read 2026-11-26)

- One policy for the whole program, read by all four settle paths:
  `Config { admin, policy: FeePolicy, fee_mode, session_fee_float,
  flats_updated_at, bump, treasury_bump, version }` (**version 4** — the three
  `network_fee_*` figures from `0007` took v4; the flats refresh was v3),
  `FeePolicy { fee_bps_below, fee_bps_above, sol_tier_lamports,
  token_tier_base_units, flat_usdc_base_units, flat_eurc_base_units, usdc_mint,
  eurc_mint, network_fee_lamports, network_fee_usdc_base_units,
  network_fee_eurc_base_units }`, plus a `FeeAnchor` PDA holding the admin's SOL
  price. `helpers::fee_for(config, amount, mint, receipt_rent, multiplier_bps)`
  (and `fee_for_signatures(..., signatures)` for a sponsored settle) is the single
  place the split is decided: a marginal rate
  (`fee_bps_below` up to the currency's boundary, `fee_bps_above` above it),
  floored at the 48-byte marker's rent plus the network fee — `max(proportional,
  flat + network)` for a token, `max(proportional − receipt_rent, network)` for
  SOL — and capped at `amount − 1`.
- `helpers::fee_for(config, amount, mint, receipt_rent, multiplier_bps)` takes
  the shop's multiplier and applies it to the percentage alone, and
  `state::ShopPolicy { seller, shop_key, multiplier_bps, bump }` is the account a
  shop's rate lives in: `set_shop_policy` writes it, `helpers::shop_multiplier`
  resolves it (or `SHOP_FEE_BASE_BPS` when the invoice names none) and refuses a
  policy recorded under another seller. All four settle contexts take it as a
  trailing `Option<Account<ShopPolicy>>`, so every invoice with no policy — and
  every one printed before this existed — still pays `Config`'s rate.
- The structure moved since this plan was drafted: `fee_bps` +
  `fixed_fee_lamports` became the tiered `FeePolicy`, and the receipt became a
  48-byte marker (`760053b`, `1c0600d`). A per-shop rate therefore has to say
  what it does to two rates, two boundaries and two token flats, not to one
  number.
- A **shop is an API row, not an account**: `shops(id, user_id, name,
  description, currency, created_at)` plus `products`, in `apps/api/src/db.rs`
  (`currency` arrived with shop-menu pricing; there is still **no** policy-address
  column). A seller may keep several shops (a café and a bakery), and the
  invoice's lines come from the shop that priced it.
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
5. **The existing `version` field is the migration hook.** `Config` carries one
   and `migrate_config` already upgrades v1 and v2 in place; the working tree
   writes version 3 (the flats refresh), so version 4 looked like the next slot.
   *(Resolved 2026-11-26: `0007` took version 4 for the network-fee fields;
   neither this plan nor the deferred `0003` needs a `Config` byte, and `0006` —
   now deferred — also uses a separate `ReceiptMode` PDA.)*

## Tasks

### Task A: Decide the shape before the code

- [x] Decide what a "shop" is on chain: a `ShopPolicy` PDA
      (`[shop-policy, seller, shop_key]`) created by the seller and confirmed by
      the admin, a seller-level `SellerPolicy` that shops share, or no policy
      account at all and an off-chain-only rate with the global fee unchanged.
      Record the decision here. — 2026-11-22: a **per-shop `ShopPolicy` PDA**
      created by its seller. The seller-level shape collapses two shops of one
      wallet into one rate, which is the opposite of "per shop"; an off-chain
      rate is unenforceable, because the seller can edit the row and keep the
      difference. The admin's hand is not needed to create one (the band is
      structural, below); an admin close comes with the setter.
- [x] Decide who may set a rate and within what band: admin-only, seller-set
      within `Config`'s range, or seller-set below a hard cap. Decide what
      happens to a shop whose rate is out of band at settle time. — 2026-11-22:
      **seller-set, `0..=10_000`**, and the cap is the type's own range plus a
      check on write, so there is no out-of-band state at settle time and no
      `Config` field to read. A shop can discount itself and never surcharge a
      buyer; anything else would need a band in `Config`, which is the version
      bump this shape avoids.
- [x] Decide the fallback: a settlement with no policy account charges
      `Config`'s rate, so every invoice printed before this lands still settles.
      — 2026-11-22: yes, and it is the same path a shop without a policy takes.
      The account is optional (`Option<Account<ShopPolicy>>`), never a new
      instruction.
- [x] If a shop policy is chosen: decide its identity — the API's 16-byte hex id
      is not an address, so it needs either a hash in the seeds or a freshly
      generated keypair whose address the API stores beside the row. —
      2026-11-22: a **freshly generated `shop_key`** stored beside the `shops`
      row, as a seed beside the seller. Hashing the row's hex id would work, but
      it binds the on-chain account to a database-generated string; a keypair
      address is a real address, can be created before the row exists, and the
      seller seed still keeps two sellers' shops apart.
- [x] Decide how a shop rate composes with the tiered policy and the token
      flats. A shop needs two rates and two boundaries, and on a token its fee
      must still cover the marker's rent or the treasury loses money on every
      cheap sale. One multiplier on `Config::policy` — keeping the flats shared
      and refreshed in one place — is the smallest shape; a full policy per shop
      means each shop needs its own flat, priced from the same anchor. —
      2026-11-22: **one multiplier**, applied to the percentage before the
      carriers. The boundaries and the two flats stay `Config`'s, so a refresh
      still has one place to write and a shop cannot undercut the record: at
      `0` a token invoice still pays the marker's flat and a SOL seller still
      hands the rent over out of the price. Two rates and two boundaries per
      shop would mean a flat per shop priced from the same anchor — the cost
      this shape exists to avoid.

### Task B: Program — policy accounts and `fee_for`

- [x] Add the policy account with `InitSpace`/space constant, its layout
      assertion beside `account_spaces_match_the_sizes_used_by_the_program`,
      and a named error for a policy that does not belong to the invoice's
      seller. — 2026-11-22: `state::ShopPolicy`, `SHOP_POLICY_SEED`,
      `SHOP_POLICY_SPACE` (75) and `SHOP_FEE_BASE_BPS` are written by
      `set_shop_policy`, `shop_policy_layout_fits_its_space` pins the struct to
      the space the setter allocates, the space arithmetic is in
      `account_spaces_match_the_sizes_used_by_the_program`, and
      `PaymentError::ShopPolicySellerMismatch` (6022) is the refusal for a
      policy recorded under another seller. The cap is on write —
      `PaymentError::ShopFeeTooHigh` (6021) — so no settlement reads an
      out-of-band rate.
- [x] Split `fee_for` into `fee_for(policy, amount)` with the policy resolved
      once per settle: the shop policy when present, `Config` otherwise. All
      four settle instructions use it — including `pay_from_budget` and
      `pay_from_token_budget`, where the invoice names the seller and the
      policy therefore travels with the invoice. — 2026-11-22: the arithmetic
      half is done (`fee_for(..., multiplier_bps)`); every path passes
      `SHOP_FEE_BASE_BPS` until the optional account is wired, which increment 2
      did: every path now resolves the multiplier with `shop_multiplier`, and
      all four contexts take the policy as a trailing optional account.
- [x] Keep the split arithmetic cap (`amount - 1`) and the
      `InvoiceTooSmall` floor identical, so a rate change cannot create an
      underflow the old one could not. — 2026-11-22:
      `a_shop_multiplier_scales_the_rate_and_never_the_record` and the unchanged
      suite pin it.
- [ ] Record the policy on the settlement: a field on `PaymentSettled` so a
      watcher does not have to guess which rate applied. The receipt is now a
      48-byte marker with no room for it; growing `PAYMENT_RECEIPT_SPACE` raises
      the rent the flats are priced from, so treat that as its own change (the
      event alone is enough for an indexer).

### Task C: Admin and configuration

*(Decided away 2026-11-22 / re-confirmed 2026-11-26: both boxes describe the
shape Task A rejected. The band is the hard `0..=10_000` cap on write in
`helpers::validate_shop_multiplier`, not a `Config` field, and there is no admin
approve/revoke path — the seller signs `set_shop_policy` within that band. Leave
these unchecked as history; do not implement them without a new decision.)*

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
      *(The relayer's half is ready: `POST /api/sponsor/settle` takes
      `shop_policy` and rebuilds the message with it, so only the caller is
      missing. No client names a policy yet.)*
- [ ] Show the seller the effective rate per shop and warn when a policy is
      missing or stale; show the buyer only the price, as today.
- [ ] Check `apps/api`'s receipt building and admin feed, which currently derive
      the fee from `Config`; they must read the event's `fee` instead.
      *(Superseded 2026-11-26: the API decodes `PaymentSettled` positionally and
      deliberately skips `fee`/`seller_proceeds`, per `0002`'s recorded decision
      in `plans/archive/0002-fee-model-and-treasury.md`; there is no `Config`-derived
      settled fee to migrate.)*

### Task E: Tests and devnet

- [x] Harness: two shops under one seller with different rates settle for
      `amount - fee_shop` each, with the treasury receiving each shop's fee. —
      2026-11-22: `two_shops_of_one_seller_settle_at_their_own_rates` (half and a
      fifth of `Config`'s rate, then a third settle with no policy for the
      fallback), plus `the_sol_path_charges_the_shop_rate` for the SOL wallet
      path and its three-lamport-accurate delta.
- [x] Harness: the allowance paths take the shop's rate too — `pay_from_budget`
      and `pay_from_token_budget`, where the policy travels with the invoice's
      seller and the pot pays the price whatever the split. — 2026-11-22:
      `the_budget_path_charges_the_shop_rate` and
      `the_token_allowance_charges_the_shop_rate`, each at half of `Config`'s
      rate, asserting the seller's share, the treasury's fee and the pot's
      outflow (and `spent`).
- [x] Harness: an invoice with no policy charges `Config`'s rate (the fallback),
      and a policy whose seller differs from the invoice's is refused by name. —
      2026-11-22: the third settlement in the two-shops test is the fallback, and
      every other test in the suite settles with no policy account at all;
      `a_policy_of_another_seller_is_refused_by_name` asserts `Custom(6022)`.
- [ ] Harness: a policy at the cap leaves the seller one base unit, as
      `a_ten_thousand_bps_policy_leaves_the_token_seller_one_base_unit` does for
      the global rate. *(Open: the write-side cap is pinned by
      `set_shop_policy_writes_the_rate_and_refuses_one_above_the_cap`, and the
      arithmetic by `a_shop_multiplier_scales_the_rate_and_never_the_record`;
      what is missing is a settlement through a policy written at `10_000`.)*
- [ ] Client: quote tests for a shop with and without a policy, and one where
      the policy is out of band.
- [ ] Devnet: set a shop rate, settle one invoice through it, and record the
      signature and the split here.

## Acceptance criteria

- [x] A settlement's fee is the rate of the shop that priced it, and the event
      agrees with the lamports that moved. (The receipt is a 48-byte marker and
      carries no fee; the event is the only on-chain record of the split.)
      *(Verified 2026-11-26 in all four harness paths.)*
- [x] An invoice with no policy, or one printed before this ships, settles at
      `Config`'s rate unchanged. *(Verified: `shop_multiplier(None)` and the
      no-policy settlement in `two_shops_of_one_seller_settle_at_their_own_rates`.)*
- [x] No seller can raise its own rate above the configured band, and no
      settlement can name another seller's policy. *(The band is the hard
      `0..=10_000` write cap; `a_policy_of_another_seller_is_refused_by_name` covers
      the second half.)*
- [x] `README.md` states that the fee is per shop, with a pointer to the band.
      *(2026-11-26: `README.md:366-376`.)*

## Verification

- [x] `cargo build-sbf --manifest-path programs/payvand/Cargo.toml`, then
      `cargo test --test settle` in `apps/contracts` (never `anchor test` here).
      *(2026-11-26: clean build, 74/74 settle tests pass; the suite had 67 at the
      last increment.)*
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
      *(2026-11-26: fmt clean; clippy fails on six test lints — see `0003`'s header.)*
- [x] `anchor idl build`, copied to `apps/web/app/assets/payvand.idl.json`.
      *(Done earlier; the IDL has since grown `0007`'s fields.)*
- [ ] `apps/api` tests for the shop row migration and the fee it reports.
- [ ] In `apps/web`: `vitest run`, `nuxt typecheck`, `eslint .`.
- [ ] Devnet smoke run recorded here.

## Dependencies

`0002` Task 4.3 (the `PaymentSettled` split the event must keep truthful) and
Task 1.6 (the fee boundary tests that must hold at any rate). *(Resolved
2026-11-26: `0003` is deferred and needs no `Config` byte, this plan's multiplier
needs none either, and `0007` took version 4 for the network-fee fields.)*

## Risks And Mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Quote and settle disagree on the rate | High | One source of truth per shop; the API reads the policy, the program enforces it, and the event reports what happened. |
| A seller sets a rate that hides its own fee | Medium | Band enforced on chain at set time and settle time, not only in the UI. |
| Old QRs stop working | High | The account is optional and the fallback is `Config`'s rate, proven by a test. |
| Indexers keep pricing from `Config` | Medium | The fee and the policy address ride in the event; `apps/api` is migrated in the same change. |
| Two plans bump `Config` at once | Medium | One bump, sequenced with `0003`, recorded in `DEV.md`. **Resolved for this plan (2026-11-22):** the multiplier shape needs no `Config` field, so `0005` takes no version and the race is gone; `0003` is deferred anyway. |

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

## Removal (not needed)

Reason: the fee is deducted from the seller's side, not charged to the buyer, so a shop lowering its rate only benefits the seller and costs the protocol; buyers always pay the listed price. The feature is not exposed in the webapp and adds complexity for no user-facing benefit.

- [ ] Remove per-shop fee multiplier from the contract: drop `ShopPolicy` PDA, `set_shop_policy`, and the optional trailing account on settle paths; settle always uses `Config`'s rate.
- [ ] Remove shop-rate handling from the API and web composables (`shopPolicy`).
- [ ] Remove related tests and keep a migration that ignores any stale shop-policy accounts.
- [ ] Remove the FAQ/README mentions of shop-set rates.
