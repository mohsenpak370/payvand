# 0003 — Token-2022 migration (`token_interface`)

Status: `Draft`. Follow-up to `0002`'s Task 4.4, which decided the program
settles SPL Token mints only and made that refusal explicit rather than leaving
it to a failure inside a CPI. `README.md` links this file as the place the
migration is tracked; it is not approved for implementation.

## What exists today (0002, revision 14)

- Every token context types its accounts as `InterfaceAccount<TokenAccount>`
  and its token program as `Interface<TokenInterface>`, so either program
  parses, and its mint as an `UncheckedAccount` constrained to
  `mint.owner == anchor_spl::token::ID && mint.data_len() == SPL_MINT_LEN` (82)
  with `PaymentError::Token2022Unsupported` (`Custom(6017)`).
- The CPIs are still `anchor_spl::token`'s (`transfer_checked`), the ATA
  constraints are still SPL Token's derive, and the treasury holds one token
  account per mint, opened by `open_treasury_token_account`.
- `a_token_2022_mint_is_rejected_by_name` asserts a Token-2022 settle fails with
  `Custom(6017)`, writes no invoice or receipt and moves no tokens. It flips in
  this plan, so it is the guard that says how far the migration got.

## Why it is not a constraint change

Dropping the owner check lets a Token-2022 mint through every layer below, and
each one changes:

1. **The split.** `settle_token_payment` pays `amount − fee` to the seller and
   `fee` to the treasury with `transfer_checked`. A transfer-fee mint takes its
   own cut in flight, so the seller would receive `amount − fee − mint fee` and
   the event's `seller_proceeds` would be a promise the token does not keep.
2. **The currency rule.** "SPL Token program" stops being the allowlist. The
   policy has to name mints (or permitted extensions) on chain, which is `Config`
   growing a field, not a rename.
3. **Accounts.** `get_associated_token_address` derives under the wrong program
   for a Token-2022 mint, on both sides of the wire: the program's ATA
   constraints, `usePaymentProgram`'s derive, the seller's setup, and the
   treasury's account. Token-2022 account sizes also vary by extension, so the
   `data_len` assumption goes.
4. **Extensions with authority over the money.** Permanent delegate, transfer
   hook, confidential transfer and mint close authority each break an assumption
   the program's accounting makes. Refusing them is a decision per extension,
   and a refused one needs its own named error like `Token2022Unsupported`.

## Tasks

### Task A: Decide the policy before the code

- [ ] Write down which extensions a settlement may see: none, a fixed allowlist
      (e.g. metadata pointer only), or per-mint opt-in through `Config`.
- [ ] Decide what a transfer fee does to the split: refuse the mint, or charge
      it — `transfer_checked_with_fee` needs the expected fee, and
      `seller_proceeds` would then be the post-fee figure.
- [ ] Record the decision here before any program edit, the way `0002`'s Task
      4.4 recorded its own.

### Task B: `Config` mint policy

- [ ] Add the mint policy to `Config` with a space bump, its layout assertion
      (`account_spaces_match_the_sizes_used_by_the_program`) and a settle test
      that a mint the policy admits still prices and splits exactly as now.
- [ ] Keep `initialize_config`/`update_config` backwards-compatible for the
      deployed `Config` (realloc + version bump, or a fresh account and a
      migration step recorded in `DEV.md`).

### Task C: Interface CPIs and the split

- [ ] Move the three token CPIs to `anchor_spl::token_interface`, keeping the
      `SellerPays` split identical for an SPL Token mint
      (`settle_token_payment`, `pay_from_token_budget`,
      `open_treasury_token_account`).
- [ ] Emit `PaymentSettled` with the same `amount`/`fee`/`seller_proceeds`
      semantics, or a field that names a mint-side fee, so `0002`'s event note
      stays true.
- [ ] Refuse a disallowed extension by name, beside
      `Token2022Unsupported`.

### Task D: Accounts on both sides

- [ ] Derive ATAs with the mint's owning program in the program's constraints
      and in `apps/web` (`usePaymentProgram`, the seller's setup, the treasury
      script `pnpm treasury:open`).
- [ ] Let the treasury hold one account per (mint, program) pair; document the
      extra step for a Token-2022 mint in `DEV.md`.

### Task E: App and API

- [ ] Offer a Token-2022 mint in `CURRENCY_MINTS` only once the treasury account
      exists on that cluster — the checkout already refuses otherwise, so the
      mint list stays the product switch.
- [ ] Check `apps/api`'s token handling (decimals, mint reads, the receipts it
      decodes) against a Token-2022 mint's larger account.

### Task F: Tests and devnet

- [ ] Flip `a_token_2022_mint_is_rejected_by_name` into the positive case: a
      Token-2022 mint with no extensions settles with the same split as SPL
      Token.
- [ ] One test per refused extension, each asserting its named error and no
      movement of tokens.
- [ ] A transfer-fee mint test if Task A admits that extension; otherwise it is
      a refusal test.
- [ ] Devnet: upgrade in place (extend `ProgramData` first if the ELF grows),
      open a treasury account for the test mint, settle one invoice through
      `pnpm smoke:silent-payment`.

## Acceptance criteria

- [ ] A Token-2022 mint the policy admits settles with the split the event
      reports.
- [ ] Every refused extension fails with its own error, before any transfer.
- [ ] An SPL Token settlement is byte-for-byte the same instruction as today
      (same accounts, same split), proven by the existing suite passing
      unchanged.
- [ ] `README.md` and `DEV.md` state the supported programs and extensions
      instead of "SPL Token only".

## Verification

- [ ] `cargo build-sbf --manifest-path programs/payvand/Cargo.toml`, then
      `cargo test --test settle` in `apps/contracts` (never `anchor test` here).
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
- [ ] `anchor idl build` regenerated and copied to
      `apps/web/app/assets/payvand.idl.json`.
- [ ] In `apps/web`: `vitest run`, `nuxt typecheck`, `eslint .`.
- [ ] Devnet smoke run recorded here.

## Dependencies

`0002` Task 4.4 (the named rejection and the interface account types) and
`0002` Task 4.3 (the events whose split a Token-2022 settle must keep truthful).

## Estimated scope

Large: it touches `Config`, three token instructions, the ATA derivation on both
sides, the treasury script, the mint list, and the harness.
