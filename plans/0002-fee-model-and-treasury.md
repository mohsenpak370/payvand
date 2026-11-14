# Plan: Fee Model, Treasury, and Budget Accounting

## Status

In Progress — new (2026-11-12), split from the 2026-11-09 contract security
draft; revision 2 (2026-11-13) notes the receipt consolidation that landed in
`0001`; revision 3 (2026-11-13) records Phase 1 Tasks 1.1 and 1.2 as built and
tested on the program, with the client half (Tasks 1.3–1.5) and any deployment
still to come; revision 4 (2026-11-14) recomputes every fee and rent figure for
the 301-byte receipt and starts Task 1.4 in the web client.

## Revision Notes

Revision 2 records one `0001` implementation change that moves this plan's
numbers:

- `0001` shipped a single `PaymentReceipt` (301 bytes, 2,985,840 lamports rent)
  keyed by the invoice address, instead of separate SOL and token receipts
  (237 and 269 bytes). `0001`'s revision notes explain why.
- Every `R_r` (receipt rent) figure in this plan therefore moves to 2,985,840:
  the treasury's per-invoice outlay, the break-even invoice (now
  2,985,840 / `fee_bps`), and the treasury delta tables.
- The money tables were recomputed from 2,985,840 in revision 4 (2026-11-14):
  break-even at 2% is 0.149292 SOL, the treasury floor before any settle is
  3,876,720 lamports, and `MIN_SESSION_FEE_FLOAT` in the program is 5,477,520.
  Keep the rent read over RPC as the single source of truth. No decision in this
  plan changes because of the consolidation.

Revision 3 also records what building Task 1.2 taught:

- The program cannot start a CPI while its current instruction frame has an
  outstanding lamport delta (`UnbalancedInstruction`). The treasury refund is a
  system transfer signed by the treasury seeds, so in the budget path it now
  runs **before** the direct lamport moves; the direct path refunds after the
  receipt write and the auto-close, which nets to zero.
- `Custom(102)` (`InstructionDidNotDeserialize`) is the error for an unknown
  `FeeMode` byte: Borsh rejects the instruction data before the handler runs,
  which is the named rejection the acceptance criteria asked for.
- The upgradeable loader's state variants are ordered `Uninitialized`, `Buffer`,
  `Program`, `ProgramData`, so the `ProgramData` bincode tag is `3`; the harness
  pins that against `bincode::serialize` instead of trusting a guess.

## Linked Docs

- `plans/README.md` — plan conventions and lifecycle.
- `plans/archive/0001-contract-security-hardening.md` — prerequisite: receipt-backed
  settlement, auto-close, and the status model. This plan assumes `0001` is
  merged and deployed.
- `README.md` — project overview.
- `apps/contracts/programs/payvand/src/state.rs` — account layouts.
- `apps/contracts/programs/payvand/src/constants.rs` — seeds, spaces, floats.
- `apps/contracts/programs/payvand/src/instructions/sol.rs` — SOL invoices and
  budgets.
- `apps/contracts/programs/payvand/src/instructions/token.rs` — token invoices
  and budgets.
- `apps/contracts/programs/payvand/src/lib.rs` — entrypoints and documented
  guarantees.
- `apps/api/src/payments.rs` — on-chain decoders, rent fields.
- `apps/web/app/composables/useBudget.ts` — builders, rent reads, UI checks.
- `apps/web/app/composables/usePaymentCheckout.ts` — direct settle builder.
- `apps/web/app/composables/useSessionKey.ts` — session-key storage and
  forgetting.
- `apps/web/app/pages/budget.vue`, `sell.vue`, `shops/[id].vue`, `pay.vue` —
  fee/rent display and preflight checks.
- `apps/web/app/assets/payvand.idl.json` — client IDL.

## State Inherited From `0001`

This plan starts from the code `0001` leaves behind:

- The payment account is created, settled, and closed in one transaction; the
  receipt PDA is the permanent record and the replay guard.
- The receipt rent is advanced by the payer (or session key) and **not**
  refunded, so a direct SOL invoice costs the payer `amount + receipt_rent`.
  Phase 1 here replaces that with the treasury refund.
- There is no `Config`, no treasury, and no protocol fee.
- `pending` / `settled` / `missing` are defined against the receipt.
- Budget settlements make the session key whole for `payment_rent +
  receipt_rent`, and the pot pays both rents on top of the invoice amount.
- One `PaymentReceipt` type covers SOL and token invoices (301 bytes,
  `["payment-receipt", invoice]`, with `Pubkey::default()` for a SOL mint), and
  every settle path writes it before closing the invoice to the seller.
- The minimum payable invoice is the payment rent (`InvoiceTooSmall`), which
  this plan raises once the fee is added.
- Deployed state: the program is upgraded on devnet under the original ID, the
  API and web app are live against it, and the devnet invoices left over from
  the old program are already settled and therefore inert.

## Objective

Make the economics explicit and correct: the payer pays exactly the price shown
on the QR, the seller bears a percentage fee deducted from its proceeds, the
protocol treasury pays the permanent receipt rent for both SOL and token
invoices, the temporary invoice rent returns automatically, and the monthly
budget records what it actually paid. The treasury is a program PDA, so no
caller can redirect the fee.

## Scope

In scope:

- A `Config` PDA (fee policy, session float) and a program-owned treasury PDA.
- Seller-pays fee split for SOL and token invoices; the fee goes to the
  treasury and nowhere else.
- Treasury-funded receipt rent for SOL **and** token invoices, refunding the
  payer (or session key) that advanced it.
- Explicit underflow guards and a minimum payable invoice amount.
- Budget accounting: the pot pays exactly the invoice amount, `spent` matches
  it, same-month top-ups preserve `spent`.
- Token fee splitting, treasury token ATA, and session-key float sizing with a
  top-up path.
- `close_token_budget` mint check and canonical vault constraint.
- Session-key sweep on close/rotate, corrected documentation, and program
  events.
- Token-2022 decision, `Config` layout test, dead-code cleanup.
- Devnet rollout of the fee model.

Out of scope:

- Anything in `0001` (close removal, receipt creation, status model).
- Payer-side gasless/relayer support: the payer still needs SOL for the network
  fee and the temporary invoice rent on token invoices until a sponsor exists.
- Token-2022 migration (documented as unsupported, tracked separately).
- Mainnet deployment, per-shop fee overrides, discounts, a full event indexer.

## Dependencies

Product decisions (adopted; the numbers below back them):

- **Fee: `fee_bps = 200` (2%), `fixed_fee_lamports = 0`.** The seller bears it.
  The payer pays the displayed price.
- **Treasury funds the receipt rent for both paths.** No `fee >= receipt_rent`
  rule, so a small invoice is payable instead of rejected after its QR was
  shown. The trade-off is a per-invoice subsidy below break-even; see "Fee And
  Rent Numbers".
- **The treasury is a program PDA, not a wallet.** `[TREASURY_SEED]`, holding
  SOL, receiving all SOL fees and paying all receipt rent. The settle contexts
  derive it from `Config.treasury_bump`, so the payer — who builds the
  transaction — has no account to substitute. Admin surplus leaves via
  `withdraw_treasury`.
- **Token fees go to the treasury's ATA for that mint**, created and funded by
  the treasury itself on first use, never by the payer.
- **For token invoices the payment account closes to whoever advanced its
  rent** — the payer for a direct payment, the session key for a budget
  payment. Revision 1 of the draft closed it to the seller, which made the
  payer's SOL a rebate to the seller; with the treasury covering receipt rent,
  the seller has no SOL exposure and needs no rebate.
- **A same-month top-up never resets `spent`.** Only a calendar-month change
  does.
- **`fee_mode` stays in `Config`** (`SellerPays` default, `BuyerPays` for future
  products) but the UI only ever sends `SellerPays`.
- **Only the program's upgrade authority can initialize `Config`**, and the
  `admin` it records is that signer. The check reads this program's `ProgramData`
  upgrade authority through the loader's own PDA, which closes the "first caller
  on a fresh devnet installs themselves" hole.

Technical decisions:

- Fee math is integer and floors in the seller's favour:
  `fee = min(amount * fee_bps / 10_000 + fixed_fee_lamports, amount - 1)` for
  SOL, `fee = min(amount * fee_bps / 10_000, amount - 1)` for tokens.
- Settle requires `amount > payment_rent + fee` (`InvoiceTooSmall`) so no
  subtraction can underflow. This replaces the removed `FeeTooLow` rule.
- The treasury must hold its own rent-exempt minimum plus the receipt rent, or
  settle fails with `TreasuryUnderfunded` before any transfer.
- `Config` is created once per deployment; the client caches it with a TTL and
  refuses to estimate costs while it is unreadable.

Open questions (do not block Phase 1, but decide before Phase 3/5):

- Who funds the treasury initially on devnet (admin wallet transfer, expected
  size 0.05–0.1 SOL), and what is the monitoring threshold before it runs dry?
- Is 2% the launch rate, or a placeholder for a smaller pilot?
- Do we ever need per-shop fee overrides, or is one global policy enough?
- Should the fee recipient stay a PDA permanently, or move to a multisig once
  the protocol has one?

## Fee And Rent Numbers

Rent uses Solana's rent-exempt formula, `(bytes + 128) × 6960` lamports. These
values are what the program and the client must agree on; tests assert them via
`Rent::minimum_balance`, and the web client already reads rent over RPC
(`useBudget.ts` → `rentMinimum`), so nothing hardcodes a stale number except
these planning figures.

| Account | Bytes | Lamports | SOL |
| --- | ---: | ---: | ---: |
| `Config` | 62 | 1,322,400 | 0.0013224 |
| `Budget` | 97 | 1,566,000 | 0.0015660 |
| `TokenBudget` | 129 | 1,788,720 | 0.0017887 |
| `Payment` | 230 | 2,491,680 | 0.0024917 |
| `TokenPayment` | 262 | 2,714,400 | 0.0027144 |
| `PaymentReceipt` (one type, both currencies) | 301 | 2,985,840 | 0.0029858 |
| Token account (ATA) | 165 | 2,039,280 | 0.0020393 |
| Treasury (0 data) | 0 | 890,880 | 0.0008909 |

`Config` = `8 + 32 admin + 2 fee_bps + 8 fixed_fee_lamports + 1 fee_mode +
8 session_fee_float + 1 bump + 1 treasury_bump + 1 version`.

### SOL invoice at 2%, treasury-funded receipt rent

Payer pays `amount` in total (the receipt rent is advanced at `init` and
refunded by the treasury in the same transaction). Seller receives
`amount - fee`. Treasury delta is `fee - R_r`, where `R_r = 2,985,840` is the
301-byte receipt.

| Invoice (SOL) | Lamports | Fee | Seller receives | Treasury delta |
| --- | ---: | ---: | ---: | ---: |
| 0.005 | 5,000,000 | 100,000 | 4,900,000 | −2,885,840 |
| 0.01 | 10,000,000 | 200,000 | 9,800,000 | −2,785,840 |
| 0.05 | 50,000,000 | 1,000,000 | 49,000,000 | −1,985,840 |
| 0.149292 | 149,292,000 | 2,985,840 | 146,306,160 | 0 |
| 0.5 | 500,000,000 | 10,000,000 | 490,000,000 | +7,014,160 |
| 1 | 1,000,000,000 | 20,000,000 | 980,000,000 | +17,014,160 |
| 10 | 10,000,000,000 | 200,000,000 | 9,800,000,000 | +197,014,160 |

Break-even invoice (fee = receipt rent), where the treasury stops subsidising:

| `fee_bps` | Break-even | In SOL |
| ---: | ---: | ---: |
| 100 (1%) | 298,584,000 | 0.298584 |
| 150 (1.5%) | 199,056,000 | 0.199056 |
| 200 (2%) | 149,292,000 | 0.149292 |

Consequences to state in the docs and the admin view:

- Invoices below 0.149292 SOL at 2% are subsidised by the treasury at
  `2,985,840 − floor(amount × 0.02)` lamports each, roughly
  `0.00299 − 0.02 × amount` SOL. That is the price of accepting small
  invoices; the alternative (rejecting them) is what revision 1 did.
- Raising `fixed_fee_lamports` to ≈3.0M (2,985,840) would make every invoice
  self-funding, but a flat 0.003 SOL fee is 30% of a 0.01 SOL payment. Keep the
  default at 0 and let `fee_bps` carry the revenue.
- The treasury must hold at least `R_r + 890,880` = **3,876,720 lamports
  (0.0039 SOL)** before a settle. One receipt type serves both currencies, so the
  SOL and token settlements have the same floor (revision 1 needed 3,431,280 for
  SOL and 3,654,000 for tokens).

### Minimum payable invoice (underflow guard)

`amount > payment_rent + fee`, with `fee = floor(amount × 0.02)`:

- Solve `amount > 2,491,680 + floor(amount × 0.02)` → the smallest passing
  amount is **2,542,531 lamports (0.0025425 SOL)**; at 2,542,530 the guard
  rejects it (`fee` 50,850 plus rent 2,491,680 equals the amount).

Revision 1's `fee >= receipt_rent` rule implied a ≈0.254 SOL floor at 1% and a
≈0.127 SOL floor at 2%. With treasury funding the hard floor drops to 0.00254
SOL, and everything between that and break-even is payable but subsidised.

### Per-flow ledger (1 SOL invoice at 2%)

Direct payment:

| Account | Lamports | Note |
| --- | ---: | --- |
| Payer | −1,000,000,000 | net after the treasury refunds `R_r` |
| Seller | +980,000,000 | `amount − fee` |
| Treasury | +17,014,160 | `fee − R_r` |
| `PaymentReceipt` | 2,985,840 locked | permanent record |

Budget payment (session key advanced `R_p + R_r`, pot reimburses both):

| Account | Lamports | Note |
| --- | ---: | --- |
| Pot | −1,000,000,000 | exactly `amount`; `spent += amount` |
| Seller | +980,000,000 | includes the `R_p` returned by close |
| Treasury | +17,014,160 | `fee − R_r` |
| Session key | 0 | rents advanced are reimbursed |
| `PaymentReceipt` | 2,985,840 locked | permanent record |

`BuyerPays` variant (Task 1.3, direct settle only — the budget path rejects it):

| Account | Lamports | Note |
| --- | ---: | --- |
| Payer | −1,020,000,000 | `amount + fee`, after the treasury refunds `R_r` and close returns `R_p` |
| Seller | +1,000,000,000 | exactly `amount` |
| Treasury | +17,014,160 | `fee − R_r` |
| `PaymentReceipt` | 2,985,840 locked | permanent record |

### Token invoice (1,000,000,000 base units at 2%)

| Account | Tokens | SOL |
| --- | ---: | ---: |
| Payer | −1,000,000,000 | network fee only, plus new seller ATA rent if it had none |
| Seller | +980,000,000 | `amount − fee_t` |
| Treasury ATA | +20,000,000 | `fee_t` |
| Treasury (SOL) | — | −2,985,840 refunding the receipt rent |

The seller needs no SOL. The payer's SOL exposure is the network fee and, on a
fresh seller, the seller's ATA rent (2,039,280) — both disclosed at checkout.

### Session-key float

Per worst-case token payment to a fresh seller:

`R_p + R_r + seller ATA = 2,491,680 + 2,985,840 + 2,039,280 = 7,516,800`
lamports (0.0075168 SOL), of which only the ATA rent (and the network fee) is
consumed. `session_fee_float = 30,000,000` (0.03 SOL) covers three such payments
(22,550,400 peak) with headroom. For SOL budget payments the peak is
`R_p + R_r = 5,477,520` and is fully reimbursed, so the same float is ample. The
program's `MIN_SESSION_FEE_FLOAT` is that same 5,477,520, so a config can never
be given a float that cannot front one payment.

## Issues Addressed

Findings 1, 2, 10, 11, 13, 14, 18, 20, 21, and 23 are closed by `0001`.

| ID | Issue | Severity | Phase |
| --- | --- | --- | --- |
| 3 | SOL budget `spent` excludes invoice rent | High | 2 |
| 4 | Refills reset the monthly counter | High | 2 |
| 5 | Token session float consumed by ATA rent | High | 3 |
| 6 | Session-key float never returned | High | 4 |
| 7 | `close_token_budget` accepts the wrong mint | Medium | 3 |
| 8 | Hidden rent/fee costs; no fee transparency | Medium | 1 and 4 |
| 9 | No events; the record is invisible to indexers | Medium | 4 |
| 12 | Vault not constrained to the canonical ATA | Medium | 3 |
| 15 | SPL Token only; no Token-2022 | Low | 4 |
| 16 | Manual account spaces | Low | 4 |
| 17 | Old session-key float stranded on rotation | Low | 4 |
| 19 | `fee >= receipt_rent` made small invoices unpayable | High | 1 |
| 20 | Fee form of the `amount − rent − fee` underflow | High | 1 |
| 22 | Fee recipient was caller-supplied (fee redirectable) | Critical | 1 |
| 24 | `Config` absent during the upgrade window fails every settle | Medium | 5 |

## Phases

### Phase 1: Config, treasury, and the SOL fee split

**Built so far (2026-11-13):** Tasks 1.1 and 1.2 are implemented on the program
and covered by the Rust harness (20 tests green, `cargo clippy --all-targets --
-D warnings` clean). The deployed devnet program is still the `0001` build, and
the web client and the copied IDL still target the `0001` account lists — they
gain the `config` and `treasury` accounts in Task 1.4, so nothing is deployable
until then.

Goal: replace the interim payer-funded receipt rent from `0001` with a
treasury-funded model, add the fee policy, and make the fee destination
impossible to redirect.

#### Task 1.1: Add the `Config` and treasury accounts

Description: Introduce `Config` (singleton PDA) and the program treasury PDA.

`Config`:

- `admin: Pubkey` — may update the fee policy and withdraw surplus. It is
  recorded at initialization as the upgrade authority that signed, never as an
  argument the caller chooses.
- `fee_bps: u16`, `fixed_fee_lamports: u64`, `fee_mode: FeeMode`.
- `session_fee_float: u64` — replaces the hardcoded `SESSION_FEE_FLOAT`.
- `treasury_bump: u8`, `bump: u8`, `version: u8`.

Instructions:

- `initialize_config(fee_bps, fixed_fee_lamports, fee_mode, session_fee_float)`
  — creates `Config` once per deployment. Only the program's upgrade authority
  may call it, and the `admin` it records is that same signer; no caller can
  install an admin of its choosing.

  Accounts: `admin: Signer<'info>` (also the payer of `Config`'s rent),
  `program_data: UncheckedAccount<'info>` with the constraint
  `seeds = [crate::ID.as_ref()], bump, seeds::program =
  bpf_loader_upgradeable::id(), owner = bpf_loader_upgradeable::id()` — which
  pins it to this program's `ProgramData` PDA and makes a decoy account
  impossible; `config` (`init`, `[CONFIG_SEED]`), `treasury: SystemAccount`
  (`[TREASURY_SEED]`), `system_program`. No `program` account is needed:
  `crate::ID` is the program address.

  Authority check, in the handler (`anchor_lang::solana_program::bpf_loader_upgradeable`):

  1. `UpgradeableLoaderState::try_from_slice(&program_data.data.borrow())`;
     anything but `ProgramData` → `ProgramNotUpgradeable`.
  2. `upgrade_authority_address == Some(admin.key())` → proceed; `None` →
     `ProgramNotUpgradeable` (a finalized program has no authority to
     bootstrap from, and a random signer must never become admin); `Some(other)`
     → `InvalidConfigAdmin`. `Signer` already proves that key signed, so no
     separate signature check is needed.

  `ProgramData` is not wrapped in `Account<ProgramData>`: the upgradeable loader
  type is not an Anchor account, so the state is deserialized explicitly as
  above.

  `admin` pays `Config`'s rent (1,322,400 lamports), so the rollout preflight
  funds it. `update_config` changes policy only and never rewrites `admin`;
  rotating the admin or recovering a lost key is an upgrade under the same
  authority, and `DEV.md` must say so. Because the check needs `ProgramData`, a
  program deployed without an upgrade authority can never initialize `Config` —
  document that the program stays upgradeable until `Config` exists.
- `update_config(...)` — admin only; validates `fee_bps <= 10_000`,
  `fee_mode` is a valid variant, and `session_fee_float` is above the configured
  floor. The `ConfigUpdated` event is added in Task 4.3.
- `withdraw_treasury(amount)` — admin only; moves surplus SOL out of the
  treasury while leaving its rent-exempt minimum. There is no user-facing path
  that touches the treasury.

Treasury: `[TREASURY_SEED]` PDA, system-owned, no data. It is referenced in
settle contexts as
`#[account(mut, seeds = [TREASURY_SEED], bump = config.treasury_bump)]`, which
is the fee-recipient constraint: the payer cannot name a different account, and
`Config` stores no replaceable recipient address.

Acceptance criteria:

- [x] `Config` exists with the fields above and `CONFIG_SPACE = 62`.
- [x] `initialize_config` succeeds only when signed by the upgrade authority,
      and records that signer as `admin`.
- [x] A funded non-authority signer fails with `InvalidConfigAdmin`.
- [x] An account that is not this program's `ProgramData` PDA fails the seeds
      and owner constraint.
- [x] A finalized program (`upgrade_authority_address == None`) fails with
      `ProgramNotUpgradeable`.
- [x] `initialize_config` can run once; a second call fails because the config
      PDA already exists.
- [x] `update_config` rejects `fee_bps > 10_000` (`InvalidFeeBps`), an
      invalid mode (`InstructionDidNotDeserialize`, because Borsh rejects the
      unknown enum byte), and a zero float (`InvalidSessionFloat`); a test
      covers each.
- [x] `withdraw_treasury` is admin-only and cannot drain the treasury below its
      rent-exempt minimum.
- [ ] The treasury address is derived from seeds in the program and in the
      client, never passed as a plain account. **Program half done**
      (`["treasury"]`, referenced only through `config.treasury_bump`); the
      client derivation lands with Task 1.4.
- [x] Token-2022 and other programs are unaffected by `Config` creation.

Verification:

- [x] Anchor test: initialize signed by the program's upgrade authority
      succeeds, and `config.admin` equals that signer.
- [x] Anchor test: initialize from a second, funded keypair fails with
      `InvalidConfigAdmin`.
- [x] Anchor test: initialize with another program's `ProgramData` fails the
      seeds/owner constraint.
- [x] Anchor test: initialize once, second initialize fails.
- [x] Anchor test: non-admin `update_config` and `withdraw_treasury` fail; admin
      update succeeds and `Config` reads back with the new policy.
- [x] Anchor test: `withdraw_treasury` leaves the rent-exempt minimum.

Files likely touched:

- `apps/contracts/programs/payvand/src/state.rs`
- `apps/contracts/programs/payvand/src/constants.rs`
- `apps/contracts/programs/payvand/src/errors.rs`
- `apps/contracts/programs/payvand/src/instructions/config.rs` (new)
- `apps/contracts/programs/payvand/src/lib.rs`
- `apps/contracts/programs/payvand/tests/settle.rs` (the Rust harness that
  replaced the old Anchor TS suite)

Dependencies: `0001` deployed.

Estimated scope: Medium.

#### Task 1.2: Seller-pays SOL settlement with treasury-funded receipt rent

Description: Implement `SellerPays` — the only mode the product uses and the
one the ledger in "Fee and Rent Numbers" describes — for both SOL settle paths.
The payer pays exactly `amount`, the seller receives `amount - fee`, the fee
goes to the treasury, and the treasury refunds the receipt rent the payer
advanced at `init`. `BuyerPays` is a separate task (1.3) so this math can land
and be verified on its own; events are Phase 4 (Task 4.3), with `msg!` logs
only here.

`settle_payment` and `pay_from_budget` changes:

- Read `Config` (PDA `[CONFIG_SEED]`); absent → `ConfigMissing`. Require
  `config.fee_mode == FeeMode::SellerPays`, else `FeeModeUnsupported` (Task 1.3
  adds the other branch).
- `fee = min(amount * fee_bps / 10_000 + fixed_fee_lamports, amount - 1)`.
- `require!(amount > payment_rent + fee, InvoiceTooSmall)` — the underflow guard
  (finding 20). Minimum at 2%: 2,542,531 lamports.
- Direct path: transfer `amount - payment_rent - fee` to the seller, `fee` to
  the treasury, close the payment account to the seller (rent back), then
  refund `receipt_rent` from the treasury to the payer.
- Budget path: the pot pays exactly `amount` — `amount - payment_rent - fee` to
  the seller, `fee` to the treasury, and `payment_rent + receipt_rent` to the
  session key as reimbursement; close the payment account to the seller.
- `require!(treasury.lamports() >= receipt_rent + treasury_rent_min,
  TreasuryUnderfunded)` before the refund.
- `Config` is read-only here; `initialize_config` is the only writer besides
  `update_config`.
- Keep `msg!` logs carrying the split so devnet settlements are readable before
  Task 4.3 adds `emit!`.

Acceptance criteria:

- [x] Direct SOL: payer balance delta is exactly `amount` plus the network fee;
      seller delta is `amount - fee`; treasury delta is `fee - R_r`.
- [x] Budget SOL: pot delta is exactly `amount`; `spent += amount`; session key
      delta is zero; seller delta is `amount - fee`.
- [x] `fee = 0` is accepted (fee-less pilot) but only via `Config`.
- [x] `amount == payment_rent + fee` fails with `InvoiceTooSmall`; one lamport
      more succeeds.
- [x] An empty treasury fails with `TreasuryUnderfunded` before any transfer.
- [x] A `Config` with `fee_mode == BuyerPays` fails with `FeeModeUnsupported`
      on both paths (until Task 1.3).
- [x] No path sends lamports to a caller-supplied fee account.
- [x] No `emit!` call is added in this task.

Verification:

- [x] Anchor test: each balance delta above at `fee_bps = 200`, plus a seller
      delta with a 1,000,000 fixed fee (the fixed fee is not repeated in every
      delta assertion).
- [x] Anchor test: `InvoiceTooSmall` boundary; `TreasuryUnderfunded` with an
      empty treasury.
- [x] Anchor test: passing an arbitrary wallet where the treasury belongs fails
      the seeds constraint.
- [x] Anchor test: `BuyerPays` config is rejected with `FeeModeUnsupported`.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/sol.rs`
- `apps/contracts/programs/payvand/src/state.rs`
- `apps/contracts/programs/payvand/src/errors.rs`
- `apps/contracts/programs/payvand/tests/settle.rs` (the Rust harness that
  replaced the old Anchor TS suite)

Dependencies: Task 1.1.

Estimated scope: Large.

#### Task 1.3: Add the `BuyerPays` fee mode

Description: Add the config-only branch that charges the fee to the payer. The
UI never sends it and no product uses it yet; it exists so a future SKU can
charge the buyer without a program upgrade.

Rules:

- Seller always receives exactly `amount`.
- Payer sends `amount + fee`; `fee` goes to the treasury.
- The payment account closes to the payer (the account that advanced its rent),
  and the treasury refunds the receipt rent to the payer. Payer net is
  `amount + fee`.
- No `amount - payment_rent` subtraction is performed, so the underflow guard
  here is `require!(amount > 0)`, not `InvoiceTooSmall`.
- `pay_from_budget` rejects `BuyerPays` with `FeeModeUnsupported`: a budget that
  silently charged a fee the UI never showed would break the `spent == amount`
  invariant. Direct settle only, documented.
- The treasury-funding guard (`TreasuryUnderfunded`) applies unchanged.

Acceptance criteria:

- [ ] Direct SOL with `BuyerPays`: payer delta `amount + fee`, seller delta
      `amount`, treasury delta `fee - R_r`.
- [ ] Budget SOL with `BuyerPays` fails with `FeeModeUnsupported`.
- [ ] No UI path can select `BuyerPays`; `estimateInvoiceCosts` (Task 1.4)
      handles the mode for completeness and shows a payer fee line only when it
      is active.
- [ ] `SellerPays` behavior is unchanged by this task.

Verification:

- [ ] Anchor test: `BuyerPays` direct ledger deltas.
- [ ] Anchor test: `BuyerPays` budget payment fails with
      `FeeModeUnsupported`.
- [ ] Anchor test: the `SellerPays` suite still passes untouched.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/sol.rs`
- `apps/contracts/programs/payvand/src/errors.rs`
- `apps/contracts/tests/payvand.ts`
- `apps/web/app/composables/useFeeConfig.ts`

Dependencies: Task 1.2.

Estimated scope: Medium.

#### Task 1.4: Client `Config` cache and a single cost estimator

Description: Add a client module that loads `Config` (cached with a TTL, e.g.
60 s) and exposes `estimateInvoiceCosts(amount, mint?)` returning the fee,
seller proceeds, payer total, temporary rent, receipt rent, treasury refund, and
network-fee estimate. Every page and preflight check reads this helper, so the
displayed numbers and the transaction math have one source.

**Built so far (2026-11-14):** the account wiring and the `Config` read are
done. `usePaymentProgram` derives `["config"]` and `["treasury"]`,
`settle_payment`, `pay_from_budget` and `open_budget` pass them, and the copied
IDL is regenerated from the program (it now also carries `Config`,
`initialize_config`, `update_config` and `withdraw_treasury`).
`useFeeConfig.ts` reads and decodes `Config` (60 s cache, refuses to quote if
the account is missing or mistagged), and the SOL budget quote takes its session
float from there, so `useBudget.ts` no longer keeps a copy of that number. The
token flows still read the program's fixed float until Phase 3 moves them to
`Config`.

`estimateInvoiceCosts` now lives beside that cache, in `useInvoiceCosts()`. It
reads `Config` and the cluster's rent (never a constant, so it follows the
program's `Rent::get()` — devnet's rent is about 73% of the mainnet table above,
and both sides agree because both ask the cluster), and reports the fee, the
seller's proceeds, the payer's total, both rents, the network-fee estimate and
the minimum payable amount. The minimum is solved from `amount > rent + fee`
rather than probed, and matches the program's floor exactly: 2,542,531 lamports
at 2% on mainnet rent, 2,769,796 for a token invoice. `usePaymentCheckout`
quotes it on load, on a currency switch and after a discount, shows the payer's
extras under the price, and refuses to submit below the floor or when `Config`
cannot be read. The budget obstacle now asks for exactly `amount`, since the pot
no longer advances either rent.

`BuyerPays` is quoted for completeness — the payer's total carries the fee and a
"protocol fee" line appears only when the policy is that mode — but no UI path
selects it yet.

The arithmetic itself lives in `app/utils/invoiceCost.ts`, away from the chain
and the Nuxt context: `feeOn`, `minimumPayable` and `invoiceCosts`, each a pure
function of a policy and rents. That is what `invoiceCost.test.ts` holds against
the program — the fee table including the fixed-fee case the contract suite
settles, the `InvoiceTooSmall` floor for 0%, 2%, 10% and 99.99% policies on both
mainnet and devnet rent, the 10,000 bps policy that admits nothing, and a large
invoice. The fee now multiplies in BigInt, like the program's `u128`, so a large
amount cannot lose a lamport to a float product. The suite runs with
`pnpm test:web` (vitest, added for this) and is part of `task verify`.

Still open: `shops/[id].vue` and `budget.vue` from Task 1.5.

**Deploy ordering is now load-bearing:** the client sends the new account lists,
so deploying the web app before the program is upgraded (and `initialize_config`
has run) makes every SOL settlement fail with a missing-account error. Task 5.1
must upgrade the program, initialize `Config`, and fund the treasury first; the
program also tolerates the reverse window (new program, old client) only after
the client deploy, so do the program first.

Acceptance criteria:

- [x] The helper reads rent over RPC (no hardcoded rent) and `Config` from
      chain, and matches the program's floor rounding.
- [x] If `Config` cannot be read, the checkout refuses to quote a total instead
      of assuming zero fee.
- [x] The helper reports the minimum payable invoice amount and blocks
      submission below it.
- [x] A unit test compares the helper's output with the program's math for
      several amounts, including the minimum and a large invoice.

Verification:

- [x] `pnpm typecheck`
- [x] Unit test for the helper's thresholds and rounding (`pnpm test:web`).
- [ ] Manual localnet: change `Config` and watch estimates refresh after the
      TTL.

Files likely touched:

- `apps/web/app/composables/useFeeConfig.ts` (new)
- `apps/web/app/composables/usePaymentProgram.ts`
- `apps/web/app/composables/useBudget.ts`

Dependencies: Task 1.1.

Estimated scope: Medium.

#### Task 1.5: Show the fee and rent split where money is decided

Description: Use `estimateInvoiceCosts` to disclose costs on the three pages
that matter:

- `sell.vue` and `shops/[id].vue`: "You receive X after the Y% fee. The buyer
  pays exactly P (plus the Solana network fee). The permanent payment record is
  paid by the protocol; the temporary invoice rent returns to you automatically."
- `pay.vue`: the price, the network fee, and — for token invoices — the seller
  ATA rent the payer covers on a fresh seller. No fee line for the payer on SOL
  invoices.
- `budget.vue`: the open/refill breakdown reflecting `session_fee_float`.

Acceptance criteria:

- [x] The three pages show the estimate from the shared helper.
- [x] No page claims "every lamport comes back" or shows a fee the payer is not
      paying.
- [x] The token checkout states the payer's SOL costs explicitly.
- [x] Amounts and labels are localized in all five locales (en, fa, fr, de, es).

Verification:

- [x] `pnpm typecheck` and `pnpm lint`
- [ ] Localnet: create an invoice and confirm the note matches the settle
      balances.

**Built so far (2026-11-14):** the buyer checkout lists the Solana network fee
and, on a fresh seller, the token-account rent a token payment would create —
with no fee line on a SOL invoice, because the seller pays it.

The seller side is one component, `InvoiceSplitNote`: "You receive X after the
Y% fee", and that the code's rent comes back on payment while the protocol pays
for the record. It sits on `sell.vue` beside the price and on a shop's page
(`ShopPayment`, over the cart total), so both quote the same policy; the shop's
note is priced in SOL only, because a cart is.

`budget.vue` shows the signature's breakdown under the limit field:
`BudgetOpenCost` lists the pot top-up, the session-key float, the account rent
(only when it has to be made) and the network fee, then the total. The rows come
from `openBudgetCost` / `openTokenBudgetCost`, which read the float from
`Config` for SOL and still from the program's constant for tokens (Phase 3 moves
them). A limit the chain could not price says so rather than showing nothing,
and the quote is debounced while the field is being typed in.

Every page reads `Config` directly; where it is missing they show nothing rather
than a guess.

Files likely touched:

- `apps/web/app/pages/sell.vue`
- `apps/web/app/pages/shops/[id].vue`
- `apps/web/app/pages/pay.vue`
- `apps/web/app/pages/budget.vue`
- `apps/web/i18n/locales/*.json`

Dependencies: Task 1.4.

Estimated scope: Medium.

#### Task 1.6: Fee and treasury tests

**Built so far:** the balance-delta, treasury and fee-mode cases landed with
Tasks 1.1 and 1.2, and the float floor is now pinned as a boundary test
(`MIN_SESSION_FEE_FLOAT` accepted, one lamport less rejected, and the constant
asserted equal to `invoice_rent + receipt_rent`). The API's program scan is
covered too: a 62-byte `Config` account is classified as "nothing to keep"
instead of taking the payment list down, while receipts and budgets still
classify.

Acceptance criteria:

- [x] Direct SOL: payer delta `amount`, seller delta `amount - fee`, treasury
      delta `fee - R_r`, receipt holds `R_r`.
- [x] Budget SOL: pot delta `amount`, `spent += amount`, session key unchanged.
- [ ] Fee rounding floors: `amount = 99` at 2% gives `fee = 1`, seller gets 98.
- [ ] `fee_bps = 10_000` behaves as documented (`amount - 1` cap; `fee_bps = 0`
      is covered).
- [ ] `withdraw_treasury` cannot empty the treasury; a settle after a withdrawal
      that drops it below `R_r + 890,880` fails with `TreasuryUnderfunded`.
      (The withdrawal cap is tested; the withdrawal-then-settle sequence is
      not.)
- [x] Passing a wallet instead of the treasury PDA is rejected.

Verification:

- [ ] `cd apps/contracts && anchor test`

Files likely touched:

- `apps/contracts/tests/payvand.ts`

Dependencies: Tasks 1.1–1.3.

Estimated scope: Small.

---

### Phase 2: Budget accounting

Goal: make `spent` mean what left the pot for the invoice, and make refills
behave the way the UI says they do.

#### Task 2.1: Pot pays exactly the invoice amount

Description: With the Phase 1 ledger, the pot pays exactly `amount` and the
session key is made whole for both rents it advanced. Replace the old
`pot >= amount + invoice_rent` check with `pot >= amount` plus the reimbursement
transfers, and drop the old single-rent refund.

Acceptance criteria:

- [ ] `pay_from_budget` debits the pot by exactly `amount`.
- [ ] The session key is reimbursed exactly `payment_rent + receipt_rent`.
- [ ] `budget.spent` increases by exactly `amount`.
- [ ] The monthly-limit check and the balance check fail with distinct errors.
- [ ] The UI's `remainingThisMonth` and `availablePot` agree on what a payment
      costs.

Verification:

- [ ] Anchor test: pot, seller, treasury, session-key deltas; `spent == amount`.
- [ ] Anchor test: a payment at the limit succeeds, one lamport over fails with
      `BudgetExhausted`.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/sol.rs`
- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/web/app/composables/useBudget.ts`
- `apps/contracts/tests/payvand.ts`

Dependencies: Phase 1.

Estimated scope: Medium.

#### Task 2.2: Preserve `spent` on same-month top-ups

Description: Reset `spent` only when `budget.month_index != current_month_index()`;
a same-month top-up raises the pot and limit and keeps the counter.

Acceptance criteria:

- [ ] Same-month `open_budget` and `open_token_budget` preserve `spent` and
      update `monthly_limit`.
- [ ] A calendar-month change resets `spent` and updates `month_index`.
- [ ] `budget.vue` copy no longer says a refill restarts the counter.
- [ ] The rotate flow preserves `spent` unless the month changed.

Verification:

- [ ] Anchor test: spend, top up, assert `spent` preserved.
- [ ] Anchor test: month change resets the counter.
- [ ] Web copy review.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/sol.rs`
- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/web/app/pages/budget.vue`
- `apps/web/app/composables/useBudget.ts`
- `apps/contracts/tests/payvand.ts`

Dependencies: Task 2.1.

Estimated scope: Small.

#### Task 2.3: Align the UI preflight with the real costs

Description: Make the obstacle checks match the program: `silentPaymentObstacle`
compares `availablePot >= amount` and the session key's SOL against
`payment_rent + receipt_rent + network fee` (both reimbursed, so only the
*peak* matters); `tokenBudgetObstacle` compares it against
`payment_rent + token receipt rent + seller ATA rent + network fee`, using the
sized float. Errors name the missing resource: pot, monthly limit, or session
key SOL.

Acceptance criteria:

- [ ] The UI never offers a budget payment the session key cannot fund.
- [ ] "Remaining" equals what the user can actually spend.
- [ ] Open/refill breakdown reflects `Config::session_fee_float`.
- [ ] Error copy distinguishes pot, limit, and float.

Verification:

- [ ] `pnpm typecheck` and `pnpm lint`
- [ ] Localnet: drain the float and confirm the UI warns before submission.

Files likely touched:

- `apps/web/app/composables/useBudget.ts`
- `apps/web/app/pages/budget.vue`

Dependencies: Tasks 2.1–2.2.

Estimated scope: Medium.

#### Task 2.4: Budget regression tests

Acceptance criteria:

- [ ] Payment at the limit succeeds; one lamport over fails.
- [ ] Same-month refill preserves `spent`; a month change resets it.
- [ ] Pot and vault balances match the documented deltas.
- [ ] A compromised session key cannot exceed the pot balance.
- [ ] Seller proceeds and treasury fee match Phase 1 for budget payments.

Verification:

- [ ] `cd apps/contracts && anchor test`

Files likely touched:

- `apps/contracts/tests/payvand.ts`

Dependencies: Tasks 2.1–2.3.

Estimated scope: Small.

---

### Phase 3: Token payments and float safety

Goal: pay token invoices without the seller holding SOL, survive fresh seller
ATAs, and stop wrong-mint or decoy-vault closes from stranding funds.

#### Task 3.1: Token fee split and treasury ATA

Description: Split the token amount `amount - fee_t` to the seller's ATA and
`fee_t` to the treasury's ATA for that mint, creating both when absent. The
treasury funds its own ATA rent via `invoke_signed`
(`create_associated_token_account` signed by the treasury seeds) so the payer
does not pay for protocol infrastructure. The treasury refunds the token receipt
rent, and the payment account closes to whoever advanced its rent.

Acceptance criteria:

- [ ] Seller receives `amount - fee_t`; treasury ATA receives `fee_t`.
- [ ] `fee_t = min(amount * fee_bps / 10_000, amount - 1)`;
      `require!(amount > fee_t)` so a seller never receives zero.
- [ ] A fresh seller ATA is created and paid for by the payer; a fresh treasury
      ATA is created and paid for by the treasury.
- [ ] Zero-value transfers are skipped rather than sent.
- [ ] The treasury refunds `R_tr` to the payer or session key.
- [ ] The seller needs no SOL before or after the payment.

Verification:

- [ ] Anchor test: seller with no ATA and no SOL receives `amount - fee_t`.
- [ ] Anchor test: treasury ATA created on first use of a mint; second payment
      reuses it.
- [ ] Anchor test: payer's SOL delta is the seller ATA rent (when new) plus the
      network fee only.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/contracts/programs/payvand/src/constants.rs`
- `apps/contracts/tests/payvand.ts`

Dependencies: Phase 1.

Estimated scope: Large.

#### Task 3.2: Size and manage the session-key float

Description: Use `Config::session_fee_float` (0.03 SOL default, see the numbers
above) instead of the hardcoded `SESSION_FEE_FLOAT`, remove
`TOKEN_BUDGET_FEE_FLOAT`, and add an explicit top-up path so a buyer can refill
the float without changing the budget limit.

Acceptance criteria:

- [ ] `open_budget` and `open_token_budget` top the session key up to the
      configured target and never take more than needed.
- [ ] `open_token_budget` no longer charges a separate SOL float beyond
      `session_fee_float`.
- [ ] The buyer can top up the float independently.
- [ ] The client's open/refill cost estimate matches the program.
- [ ] Three consecutive token payments to fresh sellers do not exhaust the
      float.

Verification:

- [ ] Anchor test: three fresh-seller token payments, float still above zero.
- [ ] Anchor test: float top-up without a limit change.
- [ ] `pnpm typecheck`

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/contracts/programs/payvand/src/constants.rs`
- `apps/contracts/programs/payvand/src/instructions/config.rs`
- `apps/web/app/composables/useBudget.ts`
- `apps/web/app/pages/budget.vue`
- `apps/contracts/tests/payvand.ts`

Dependencies: Tasks 3.1, 1.1.

Estimated scope: Medium.

#### Task 3.3: Mint and vault constraints

Description: Add `require_keys_eq!(mint.key(), budget.mint, MintMismatch)` to the
token close/pay paths, and constrain the vault to the canonical ATA
(`associated_token::mint = mint`, `associated_token::authority = budget`) in
`pay_from_token_budget` and `close_token_budget`, matching
`open_token_budget`.

Acceptance criteria:

- [ ] A wrong-mint close fails with `MintMismatch` and strands nothing.
- [ ] A decoy token account for the same mint and authority is rejected.
- [ ] The canonical vault is the only accepted vault; its rent returns to the
      buyer on close.

Verification:

- [ ] Anchor test: wrong-mint close fails; decoy vault rejected.
- [ ] Anchor test: canonical vault drained and closed.

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/token.rs`
- `apps/contracts/tests/payvand.ts`

Dependencies: Task 3.2.

Estimated scope: Small.

#### Task 3.4: Token regression tests

Acceptance criteria:

- [ ] Wrong-mint close fails; decoy vault rejected.
- [ ] Fresh seller: ATA created, payer pays its rent, seller receives
      `amount - fee_t`.
- [ ] Treasury ATA receives `fee_t` and its rent is not charged to the payer.
- [ ] Payment account closed to the rent payer, not the seller.
- [ ] A settled token invoice cannot be replayed.
- [ ] Token monthly limit enforced and preserved across a same-month top-up.

Verification:

- [ ] `cd apps/contracts && anchor test`

Files likely touched:

- `apps/contracts/tests/payvand.ts`

Dependencies: Tasks 3.1–3.3.

Estimated scope: Small.

---

### Phase 4: Session-key lifecycle, events, and cleanup

Goal: stop losing the float, make the program observable, and close the
remaining low-severity findings.

#### Task 4.1: Sweep the session key before forgetting or rotating

Description: The program cannot move lamports out of the session key (it is not
a PDA), so the client signs `system_program::transfer` from the session key to
the buyer for `balance - fee` before `forgetSessionSigner` and before rotating.
If the sweep fails, do not forget the key; show the balance and a retry.

Acceptance criteria:

- [ ] Closing the last budget sweeps the float back before the key is forgotten.
- [ ] Rotating sweeps the old key when this device holds it.
- [ ] A rotated-out device detects it is no longer the budget's `session_key`
      and offers to sweep.
- [ ] The success message only claims "everything came back" when the sweep
      succeeded.
- [ ] A non-zero balance shows a visible recover action.

Verification:

- [ ] Localnet: close all budgets, session-key balance is zero minus the fee.
- [ ] Localnet: rotate and confirm the old balance is swept.

Files likely touched:

- `apps/web/app/composables/useSessionKey.ts`
- `apps/web/app/composables/useBudget.ts`
- `apps/web/app/pages/budget.vue`

Dependencies: Phase 2.

Estimated scope: Medium.

#### Task 4.2: Correct the fee and rent documentation

Description: Fix claims that the new model makes false:
`lib.rs` "the buyer gets every lamport back", `lib.rs` "every token and lamport
comes back", `budget.vue` "everything left went back to your wallet", and the
fee/rent note on `sell.vue` / `shops/[id].vue` / `pay.vue`. Document: payer pays
the price; seller bears `fee_bps`; the treasury pays the permanent receipt rent;
temporary rent returns automatically; token fees are deducted in the token, so
sellers need no SOL.

Acceptance criteria:

- [ ] Doc comments match behavior.
- [ ] UI success messages are accurate.
- [ ] `README.md` documents the fee model, the treasury, and the break-even
      economics from "Fee And Rent Numbers".
- [ ] `DEV.md` documents `initialize_config`, `update_config`,
      `withdraw_treasury`, and the treasury funding step.

Verification:

- [ ] Doc review.
- [ ] Manual close/rotate and invoice flows.

Files likely touched:

- `apps/contracts/programs/payvand/src/lib.rs`
- `apps/web/app/pages/budget.vue`, `sell.vue`, `shops/[id].vue`, `pay.vue`
- `README.md`
- `DEV.md`

Dependencies: Task 4.1.

Estimated scope: Small.

#### Task 4.3: Program events

Description: Emit events for every state change, with the fee/rent split so an
indexer can reconstruct proceeds without re-deriving `Config`. This task owns
the settle events that Tasks 1.2 and 1.3 deliberately left as `msg!` logs, so
the fee math could land and be verified before any event surface was added:

- `PaymentSettled` / `TokenPaymentSettled` — amount, fee, seller proceeds,
  receipt rent, rent returned, payer, receipt, mint (token), timestamp.
- `ConfigUpdated`, `BudgetOpened`, `BudgetClosed`, `TokenBudgetOpened`,
  `TokenBudgetClosed`, `TreasuryWithdrawn`.

Acceptance criteria:

- [ ] Events exist for each state change and are in the IDL.
- [ ] Settle events carry the split.
- [ ] No event describes a close path; none exists.
- [ ] `msg!` logs remain for human-readable explorers.
- [ ] The settle events replace the interim logs from Tasks 1.2 and 1.3, with
      no duplicate emission.

Verification:

- [ ] Anchor test asserts event payloads, including the `SellerPays` and
      `BuyerPays` splits.
- [ ] `anchor build` regenerates the IDL with the events.

Files likely touched:

- `apps/contracts/programs/payvand/src/events.rs` (new)
- `apps/contracts/programs/payvand/src/instructions/*.rs`
- `apps/contracts/programs/payvand/src/lib.rs`
- `apps/web/app/assets/payvand.idl.json`
- `apps/contracts/tests/payvand.ts`

Dependencies: Tasks 1.2, 1.3, and Phase 3.

Estimated scope: Medium.

#### Task 4.4: Token-2022 decision and `Config` layout test

Description: Document that the program uses `anchor_spl::token` and rejects
Token-2022, link a follow-up plan for `token_interface` migration, and make sure
the UI/API do not offer Token-2022 mints. Add an `InitSpace` (or size-assert)
test for `Config` and the budget accounts; the receipt and payment layouts keep
their `0001` tests.

Acceptance criteria:

- [ ] Product docs state supported token programs and link the follow-up.
- [ ] A Token-2022 mint fails with a clear error.
- [ ] `Config`, `Budget`, and `TokenBudget` sizes are asserted by a test.

Verification:

- [ ] Doc review.
- [ ] `cargo test -p payvand`

Files likely touched:

- `README.md`
- `DEV.md`
- `apps/contracts/programs/payvand/src/state.rs`
- `apps/contracts/programs/payvand/src/constants.rs`

Dependencies: Phase 1.

Estimated scope: Small.

#### Task 4.5: Remove dead code

Acceptance criteria:

- [ ] No unused error variants (`FeeTooLow` is gone with the rule that used it).
- [ ] No stale comments describing the old payer-funded receipt rent, the
      seller-closed token rent, or a reclaim flow.
- [ ] `cargo clippy -p payvand -- -D warnings` is clean.

Verification:

- [ ] `cargo clippy -p payvand -- -D warnings`
- [ ] `cargo check -p payvand`

Files likely touched:

- `apps/contracts/programs/payvand/src/errors.rs`
- `apps/contracts/programs/payvand/src/instructions/*.rs`
- `apps/contracts/programs/payvand/src/lib.rs`

Dependencies: Phases 1–4.

Estimated scope: Small.

---

### Phase 5: Verification, rollout, and archival

#### Task 5.1: Full verification

Acceptance criteria:

- [ ] `task verify` passes, including `anchor test`.
- [ ] `cargo clippy -p payvand -- -D warnings` passes.
- [ ] The IDL is regenerated and copied to
      `apps/web/app/assets/payvand.idl.json`.
- [ ] API tests cover the receipt rent and fee fields.
- [ ] `estimateInvoiceCosts` is tested against the program's math, including the
      minimum amount and the break-even point.
- [ ] Localnet smoke test of all four settlement paths.

Verification:

- [ ] `task verify`
- [ ] `cd apps/contracts && anchor test`

Files likely touched:

- `apps/web/app/assets/payvand.idl.json`
- Files changed in Phases 1–4.

Dependencies: Phases 1–4.

Estimated scope: Medium.

#### Task 5.2: Devnet rollout

Description: The `Config` account does not exist until after the program is
upgraded, so any settle between those two steps fails with `ConfigMissing`.
Run the upgrade and `initialize_config` in one script with a preflight check, and
keep the window under a minute.

Rollout order:

1. Preflight: confirm the upgrade authority, back up the program keypair, fund
   that authority's wallet for the `Config` rent (1,322,400 lamports) and the
   treasury seed balance (≥ 3,431,280 lamports). The authority wallet becomes
   `Config.admin`.
2. `anchor build`, regenerate and copy the IDL, then `anchor upgrade` with the
   unchanged program ID.
3. Immediately run `initialize_config(200, 0, SellerPays, 30_000_000)` signed by
   the upgrade authority, and assert `Config` reads back with
   `admin == authority`. A signature from any other key fails with
   `InvalidConfigAdmin`; if initialization fails, `anchor upgrade` back to the
   previous build.
4. Deploy the API.
5. Deploy the web app.
6. Smoke-test.

**Built so far (2026-11-14):** steps 1–3 ran against devnet, in three commands
rather than one script, so the upgrade→`initialize_config` window was a few
minutes instead of the planned one; no payment traffic was possible in it, and
nothing was paying at the time.

- Program `4grKZpw6G2zmxv62iKzr8vCJU2g3oAArpcJEeTFimq3J` upgraded in place to
  the 0002 build (491,000 bytes, same ID, same authority `Evmo7zn…`, ProgramData
  `8GQRqbdm…`, slot 522160416). Upgrade signature `2GVaAF15…`.
- `Config 8FdnWvwzEKaWHgB3nY7AZwmiKtwKDkHabDKg8x617Msy` created by the upgrade
  authority (`4PJrZ2VN…`) and then set to the rollout policy
  `(200, 0, SellerPays, 30_000_000)` (`2avp93Dj…`); it reads back exactly that.
- `Treasury 9tyu74VybRdMEZ2zXszcsqS2mhsdr1iExaxoVkXVvQc4` funded with
  50,000,000 lamports. It held 67,820,680 after the settle below, so the
  threshold to keep is one receipt rent: top up below ~0.01 SOL, which is
  2,179,320 lamports of devnet rent plus margin.
- Settle smoke (1 SOL, throwaway buyer, memo `devnet smoke`, signature
  `5gHeXsFE…`): seller +980,000,000 (`amount - fee`), treasury +17,820,680
  (`fee - receipt_rent`; devnet's receipt rent is 2,179,320, not the mainnet
  2,540,400 this table assumed), buyer −1,000,005,000 (`amount` + the 5,000
  lamport network fee, receipt rent refunded). The 301-byte receipt exists and
  the invoice account is closed.
- The app's own helper, reading that `Config` and devnet rent, quoted
  `fee 20,000,000`, `sellerReceives 980,000,000`, `minPayable 1,855,756` — the
  same numbers the settle produced. A shop page then rendered "You receive
  0.01176 SOL after the 2% fee" and the 0.00181864 SOL rent for a 0.012 SOL
  cart.

**Web redeploy (2026-11-14):** the production build that quotes from `Config`
and sends the treasury account is live at `https://payvand-app.vercel.app`
(deployment `payvand-ahj2ckg40-…`, built from the two-file upload tree the
README describes). The deployed `/budget` chunk contains the `budget.cost.*`
keys of the new breakdown, so the fee UI shipped with it.

Smoke tests:

- [x] SOL invoice at 0.01 SOL: payer pays 0.01 exactly (plus the 5,000 lamport
      network fee); seller receives 0.0098; treasury loses 1,979,320 lamports
      (fee 200,000 − receipt rent 2,179,320, devnet rent); the 301-byte receipt
      holds 2,179,320. Signature `5dq5uKBU…`.
- [x] SOL invoice at 1 SOL: seller receives 0.98; treasury gains the fee less the
      receipt rent (17,820,680 on devnet rent, not the 17,459,600 the mainnet
      table predicted).
- [x] Budget payment: pot decreases by exactly the price (10,000,000), `spent`
      becomes 10,000,000, the session key pays only the 5,000 network fee (the
      pot reimburses it both rents), seller +9,800,000, treasury +200,000 −
      receipt rent. `open_budget` moved exactly `Config.session_fee_float`
      (30,000,000) to the session key and the limit into a 97-byte account.
      Signatures `3Q7M8y4b…`, `2T48sVvp…`.
- [ ] Token invoice from a fresh seller with no SOL: seller receives
      `amount - fee_t`; treasury ATA receives `fee_t`; payer pays only the
      network fee and the new seller ATA rent. **Blocked on Phase 3:** the
      deployed `settle_token_payment` has no `config`/`treasury` accounts (see
      the IDL) and transfers the full `amount`; there is no `fee_t` to assert
      yet.
- [ ] Wrong-mint close fails; decoy vault rejected. **Blocked on Phase 3**
      (Task 3.4); not exercised.
- [x] `close_budget` returns the pot and the account rent to the buyer and the
      account is gone (`42FiDA7V…`; the stand-in budget `5Jirbyfw…` likewise
      closed a 38,963,680 lamport account back to the wallet).
- [ ] Close all budgets: the float is swept and the vault drained. Not run, and
      the SOL build does not sweep the float: `close_budget` closes only the
      budget account, so the session key keeps its 30,000,000 float less
      network fees. Reading the key's balance after a close is also unchecked,
      and `close_token_budget`'s vault drain needs a token budget (Phase 3).
- [x] Shop page estimates match the settled balances: the 0.012 SOL cart showed
      "You receive 0.01176 SOL after the 2% fee" and `0.00181864 SOL` rent.
- [x] Budget page estimates match the settled balances: `/budget` rendered
      `0.03 SOL` on the session key (the `Config` float), `0.001143 SOL` account
      rent and `0.00001 SOL` network fee, total `0.131153 SOL` for a 0.1 SOL
      limit; for a live budget it showed the settled account's limit
      `0.02 SOL`, `0.02 SOL` left, pot `0.03782068 SOL` and session key
      `Gcnh…NQpe`, matching `open_budget` `66R7jHdF…`. The Chromium preview has
      no wallet extension, so this screen used a registered wallet-standard stub
      (read-only, no signing); `close_budget` `5Jirbyfw…` removed the stand-in
      budget.
- [ ] Checkout page estimates match the settled balances. Not run: the checkout
      screen was never opened, so its fee and seller-token-account lines have
      not been compared with a settled payment.
- [x] `withdraw_treasury` moves surplus and leaves the rent-exempt minimum:
      withdrawing everything above it left 650,240 lamports (devnet's
      `minimum_balance(0)`, not the 890,880 this plan assumed for mainnet); the
      treasury was then refunded to 50,650,240. Signature `3y9KL8DJ…`.

Acceptance criteria:

- [x] The program ID is unchanged.
- [x] `Config` exists with the launch policy and is readable by the client.
- [x] The IDL in the web app matches the deployed program.
- [ ] Smoke tests pass.
- [x] The treasury balance is recorded after rollout, with a documented top-up
      threshold (67,820,680 lamports; top up below ~0.01 SOL).

Verification:

- [ ] `solana program show <PROGRAM_ID>`
- [ ] `solana account <CONFIG_PDA>` and `<TREASURY_PDA>`
- [ ] The checklist above.

Files likely touched:

- `apps/web/app/assets/payvand.idl.json`
- `apps/api` deployment config (if any)
- `README.md` / `DEV.md`

Dependencies: Task 5.1.

Estimated scope: Medium.

#### Task 5.3: Update docs and archive

Acceptance criteria:

- [ ] `README.md` documents the fee model, the treasury, and the numbers above.
- [ ] `plans/README.md` lists this plan as completed and archived.
- [ ] The plan file moves to `plans/archive/`.
- [ ] Follow-ups (Token-2022, relayer/sponsor, per-shop fees) are captured as
      issues or new plans.

Verification:

- [ ] Doc review.
- [ ] `git status` shows only intended changes.

Files likely touched:

- `README.md`
- `plans/README.md`
- `plans/archive/0002-fee-model-and-treasury.md`

Dependencies: Task 5.2.

Estimated scope: Small.

---

## Checkpoints

- [ ] **Checkpoint 1:** `Config` and the treasury exist and only the upgrade
      authority could create `Config`; SOL fees land in the treasury; the
      receipt rent is refused unless the treasury can pay it; `SellerPays` pays
      the payer exactly the price and lands before `BuyerPays` is added; small
      invoices fail with `InvoiceTooSmall` instead of underflowing; a
      caller-supplied fee account is rejected.
- [ ] **Checkpoint 2:** The pot pays exactly the invoice amount; `spent` matches
      it; same-month refills preserve the counter; preflight checks match the
      program.
- [ ] **Checkpoint 3:** Token sellers need no SOL; token fees reach the treasury
      ATA; the float covers three fresh-seller payments; wrong-mint and decoy
      vaults fail.
- [ ] **Checkpoint 4:** Closing or rotating sweeps the float; events carry the
      split; Token-2022 is documented; `Config` layout is tested; no dead code.
- [ ] **Checkpoint 5:** `task verify`, `anchor test`, and the devnet smoke tests
      pass; the treasury has a recorded balance and threshold; the plan is
      archived.

## Risks And Mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Treasury runs dry, blocking settles | High | `TreasuryUnderfunded` fails before transfers; expose the balance and a top-up threshold; `withdraw_treasury` leaves the rent reserve. |
| Below-break-even invoices drain the treasury faster than fees refill it | High | Break-even table documented; admin visibility; `fee_bps` is the lever; the minimum viable invoice is enforced on chain. |
| `Config` missing during the upgrade window | Medium | Upgrade and `initialize_config` run as one script with a preflight check and a rollback step. |
| Fee redirect through a substituted account | Critical | The fee destination is a PDA derived from `TREASURY_SEED`; no `Config` field names a replaceable wallet; a negative test passes an arbitrary wallet. |
| Fee/cap misconfiguration (10 000 bps, huge fixed fee) | High | `update_config` validates `fee_bps <= 10_000`; settle caps at `amount - 1`; admin-only writes; tests for both bounds. |
| A stranger initializes `Config` first and installs themselves as admin | Critical | `initialize_config` requires the upgrade authority's signature and verifies it against this program's `ProgramData`; the account is pinned to the loader's PDA by seeds and owner, and a non-authority signer gets `InvalidConfigAdmin`. |
| `BuyerPays` is enabled by mistake and the UI never disclosed the fee | Medium | The UI cannot select the mode; the budget path rejects it outright; `estimateInvoiceCosts` renders a payer fee line whenever the mode is active. |
| Token receipt rent refund is skipped when the treasury ATA is created | Medium | Treasury funds its own ATA; refund and ATA creation are separate transfers, both asserted in tests. |
| Float top-up turns into another hidden charge | Medium | `session_fee_float` is shown in the open/refill breakdown and is refunded on close/rotate. |
| UI estimate drifts from program math | Medium | One helper, floor rounding documented, unit test comparing it to program outputs. |
| Admin key compromise drains the treasury | High | Document the authority; move to a multisig when available; `withdraw_treasury` is the only outflow path and leaves the reserve. |

## Definition Of Done

- [ ] All listed findings are checked.
- [ ] `task verify`, `anchor test`, and
      `cargo clippy -p payvand -- -D warnings` pass.
- [ ] The payer pays exactly the displayed price for SOL invoices; the seller
      receives the price minus `fee_bps`; the treasury pays the receipt rent.
- [ ] No path can send the fee anywhere but the treasury PDA.
- [ ] Token sellers need no SOL; token fees reach the treasury ATA; the float is
      sized, topped up on demand, and swept on close/rotate.
- [ ] `spent` equals what left the pot; same-month refills preserve it.
- [ ] The three pages show the estimate the program will produce.
- [ ] Devnet smoke tests pass on an unchanged program ID with `Config`
      initialized.
- [ ] Docs and comments match deployed behavior.
- [ ] The plan is archived and listed in `plans/README.md`.

## Open Questions

- Is 2% the launch rate, or should the devnet config use a smaller pilot value?
- Initial treasury funding amount and the alert threshold before a top-up is
  required?
- Per-shop fee overrides now, later, or never?
- Should `Config.admin` (and therefore `update_config` and
  `withdraw_treasury`) move to a multisig before mainnet? The treasury itself
  stays a PDA; only the authority over it would change.
- Should token invoices ever close rent to the seller again (a deliberate SOL
  onboarding rebate), or is the payer/session-key close final?
- Payer-side gasless support remains a separate plan; confirm it is deferred.
