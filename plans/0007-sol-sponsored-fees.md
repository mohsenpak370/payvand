# 0007 — SOL sponsored fees: the seller pays the network fee there too

Status: `Done` on devnet, with two open follow-ups. The program is upgraded to v4
(`settle_payment` has a required `sponsor`, `Config` carries the three
`network_fee_*` figures), the relayer's SOL route rebuilds and fee-pays
`settle_payment`, and the checkout asks the relayer first for both currencies.

The policy write is done: `Config` was migrated to 184 bytes on 2026-11-24 and
`set_fee_price_anchor` priced it at 5 000 lamports per signature. With a funded
relayer (`85odspCg…mDmz9x`) a 50 000 000-lamport sponsored settle moved the payer
by exactly −50 000 000 and left the relayer whole at 0, while the self-sponsored
one moved its payer by exactly −50 000 000; the two seller credits differ by
exactly one signature's fee. Evidence and signatures are in `DEV.md`.

The treasury's balance staying put was investigated and is correct: the SOL floor
is `max(rate − receipt rent, network × signatures)`, and on a 50 000 000-lamport
invoice the rate (500 000) is below the receipt's rent (894 080), so the fee
floors at 10 000 and `fee − network` is zero. The transaction's inner instructions
show the payer sending 47 277 280 to the seller and 2 722 720 to the relayer —
both rents plus the network fee — and the invoice account closing back to the
seller. The check had used `max(rate, network × signatures)`; it now derives the
fee from `fee_for`'s own rule and asserts the treasury and seller shares. Details
in `DEV.md`.

The token paths are verified too: `scripts/devnet-token-fee-check.mjs` settled a
5 000 000-base-unit USDC invoice self-sponsored and sponsored, ten assertions
passing — the payer's tokens fall by exactly the price, the seller receives the
price less `max(rate, flat + network_t × signatures)`, the treasury gains exactly
63 133 / 63 484 base units, the payer's lamports do not move and the relayer ends
whole. Numbers are in `DEV.md`. All three paths in the change are now confirmed
on devnet.

## Linked Docs

- `README.md` — fee and sponsorship sections.
- `plans/0004-relayer-sponsored-fees.md` — the token relayer this plan reuses.
- `apps/contracts/programs/payvand/src/helpers.rs` — `fee_for`.
- `apps/contracts/programs/payvand/src/instructions/sol.rs` — the two SOL settles.

## Objective

Make the SOL paths charge the network fee to the seller, the way the token paths
already do, without charging it twice. Today a SOL payer is its own transaction's
fee payer: it spends ~5,000 lamports on top of the price, and the seller is
charged only the permanent record's rent. The rule we want is one rule for both
currencies — **the seller's deduction is `max(platform_rate, record_rent +
network_fee)`, and the platform keeps the difference** — so a SOL invoice must
have a fee payer that is not the payer, and the seller's proceeds must fund that
fee.

## Scope

In scope:

- An optional `sponsor` account on `settle_payment` and `pay_from_budget`.
- The SOL fee floor becoming `record rent + network fee`, with the treasury
  reimbursing the sponsor the fee it paid.
- The relayer building and signing SOL settles as fee payer.
- Client: SOL checkouts ask the relayer first, exactly as token checkouts do.

Out of scope:

- Anonymous (signed-out) sponsorship; that stays behind `auth::require_user`.
- Any change to the platform rates, the tiers or the token paths.

## Dependencies

- Product decisions:
  - Whether a SOL payment with no relayer available falls back to the payer
    paying its own fee (today's behaviour) and the seller's floor staying at the
    record rent, or is refused. Recommendation: fall back, and say so on screen.
- Technical decisions:
  - The reimbursement currency is SOL, paid from the treasury pool, and the
    seller's deduction repays it in lamports. No conversion is involved.
  - `network_fee_lamports` is a policy value, so it must be at least the real
    signature cost; overpaying the relayer is a small, bounded loss, underpaying
    it makes sponsorship unprofitable. Keep it at the configured estimate.
- Open questions:
  - Should the program refuse a `sponsor` that is also `payer`? It cannot tell
    who actually paid the transaction's fee, so this is a client-side invariant,
    not an on-chain one (see Risks).

## Issues Addressed

| ID | Issue | Severity | Phase |
| --- | --- | --- | --- |
| 1 | A SOL payer pays the network fee, so a zero-SOL wallet cannot pay a SOL invoice at all | High | 1 |
| 2 | The seller's SOL deduction is `max(rate, rent)` while the token one is `max(rate, rent + fee)`: the two currencies price the same payment differently | High | 1 |
| 3 | Charging the SOL seller a network fee the payer already paid would collect it twice | High | 1 |

## What already landed

As of `CONFIG_VERSION = 4` the rule is on chain for both currencies.

The token half:

- `FeePolicy` carries `network_fee_lamports`, `network_fee_usdc_base_units` and
  `network_fee_eurc_base_units`; the token figures are re-pegged with the flats
  by `refresh_fee_price` / `set_fee_price_anchor`.
- `fee_for` floors a token fee at `flat + network fee`, so a small payment pays
  exactly those costs and the protocol keeps nothing, and a large one pays the
  percentage out of which both costs come.
- Both token settle paths refund `invoice rent + receipt rent + network fee` to
  the sponsor, so its net cost is zero.
- `withdraw_treasury` reserves that larger refund.

The SOL half:

- `settle_payment` takes a required `sponsor` (trailing, before the optional
  `shop_policy`) that funds both records; `pay_from_budget`'s session key keeps
  that role, so its account list did not change.
- The SOL `fee_for` branch is `max(rate - receipt rent, network fee)`, so the
  seller's deduction is `max(rate, receipt rent + network fee)` and the platform
  keeps `fee - network fee`.
- The split, not the treasury, repays the sponsor: the price pays the seller
  `amount - invoice rent - receipt rent - fee`, the sponsor
  `invoice rent + receipt rent + network fee`, and the treasury
  `fee - network fee`. Those three sum to exactly `amount`, so the payer spends
  the price and nothing else, and no cost is charged or paid twice.
- Nothing leaves the treasury on a SOL settlement, so there is no underfunding
  path and no `ensure_treasury_funded` call on these two instructions.

Not yet: the API has no SOL settle route, so the web passes the wallet as its own
sponsor (Phase 2, Task 3–4). Until then a SOL payer must hold the two rents for
the length of its own transaction.

## Phases

### Phase 1: The program takes a sponsor on the SOL paths

#### Task 1: Optional trailing `sponsor` account

Description: add `#[account(mut)] pub sponsor: Option<UncheckedAccount<'info>>`
as the last account of `SettlePayment` and `PayFromBudget`. Optional and
trailing for the same reason `shop_policy` is: every message built before this
landed still works, and `None` means "the payer is the fee payer", which is what
the old behaviour assumed.

Acceptance criteria:

- [ ] With no sponsor, the instruction settles byte-for-byte as it does today.
- [ ] With a sponsor, the fee payer named by the client receives the refund.
- [ ] A sponsor equal to the payer is accepted (it is the unsponsored case).

Files likely touched:

- `apps/contracts/programs/payvand/src/instructions/sol.rs`
- `apps/contracts/programs/payvand/src/lib.rs`

Dependencies: none.

Estimated scope: Small.

#### Task 2: The SOL floor includes the network fee

Description: in the SOL branch of `fee_for`, return
`max(proportional.saturating_sub(receipt_rent), network_fee_lamports)`. The
seller's deduction is then `receipt_rent + fee = max(rate, receipt_rent + net)`;
the treasury receives `fee` and pays `net` to the sponsor, so what it keeps is
`max(rate - receipt_rent - network fee, 0)`, and a small payment leaves it with
nothing. Guard the payout with `ensure_treasury_funded(treasury, net)`, and pay
it as a `pay_from_treasury` CPI **before** the direct lamport moves in
`pay_from_budget`, so no lamport delta is outstanding across the CPI.

Acceptance criteria:

- [ ] `fee_for(None)` returns at least `network_fee_lamports`.
- [ ] A settlement small enough for the rate to be below the costs leaves the
      treasury exactly `net` in and `net` out, and the seller down
      `receipt_rent + net`.
- [ ] A large settlement leaves the treasury `rate - receipt_rent - net`, the
      seller down `rate`, and the sponsor whole including the fee it paid.
- [ ] No path can pay the fee out without having charged it (the treasury's
      lamports never fall by more than `fee` in one instruction).

Files likely touched:

- `apps/contracts/programs/payvand/src/helpers.rs`
- `apps/contracts/programs/payvand/src/instructions/sol.rs`
- `apps/contracts/programs/payvand/tests/settle.rs`

Dependencies: Task 1.

Estimated scope: Medium.

### Phase 2: The relayer builds SOL settles

#### Task 3: A SOL settle message in the relayer

Description: `sponsor.rs` today rebuilds only token settles. Add the SOL shape:
the relayer is the message's fee payer, the payer still signs, the sponsor
account is the relayer's own key, and `settle_payment`'s account list gains that
key. Keep the existing authentication and daily caps; a SOL settle costs less
than a token one, so no cap change is needed.

Acceptance criteria:

- [ ] `POST /api/sponsor/settle` accepts a SOL settle and returns a message
      whose fee payer is the relayer.
- [ ] A signed-out caller is still refused (`auth::require_user`).
- [ ] The relayer refuses a message that names a sponsor other than itself.

Files likely touched:

- `apps/api/src/sponsor.rs`
- `apps/api/src/payments.rs` (decoding, if the fee payer is reported)

Dependencies: Phase 1.

Estimated scope: Medium.

#### Task 4: The client asks for a relayer on SOL too

Description: `useCheckoutPayment` currently calls `relayerAddress()` only on the
token branch. Make it call it for SOL as well, pass the relayer as the sponsor
account, and fall back to the wallet as sponsor (today's behaviour, with the
seller's floor staying at the record rent) when no relayer is configured. The
checkout's cost panel then shows the network fee as covered on both currencies.

Acceptance criteria:

- [ ] A wallet holding only the invoice amount pays a SOL invoice end to end.
- [ ] With no relayer, the payment still settles and the seller is not charged a
      fee it did not pay.
- [ ] The cost panel's fee line matches the program's numbers in both cases.

Files likely touched:

- `apps/web/app/composables/useCheckoutPayment.ts`
- `apps/web/app/composables/usePaymentApi.ts`
- `apps/web/app/components/PaymentCheckout.vue`
- `apps/web/i18n/locales/*.json`

Dependencies: Task 3.

Estimated scope: Medium.

## Checkpoints

- [ ] Phase 1: `cargo test --manifest-path apps/contracts/Cargo.toml` passes, and
      the treasury's books are asserted in both the small and the large case.
- [ ] Phase 2: one sponsored and one unsponsored SOL settle land on devnet, and
      the payer's lamport balance is unchanged in the sponsored one.
- [ ] Before completion, `README.md`'s buyer row and sponsor paragraph are
      updated, and `DEV.md` records the SOL relayer.

## Risks And Mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| The program cannot see who paid the transaction fee, so a client could name its own key as sponsor and take the reimbursement | Low (bounded by `MAX_NETWORK_FEE_LAMPORTS` per settlement, and the seller's own app builds the message) | The account is optional and only the app's builder sets it; document that the sponsor is a client-side invariant, and keep the cap tight |
| Charging the fee to a seller whose payer paid it anyway | High | Do not change the SOL branch until the sponsor account exists, and keep `network fee == 0` in the deployed policy until Phase 1 ships |
| The treasury pays `net` before the seller's fee arrives, so a drained pool refuses settles | Medium | `ensure_treasury_funded(net)` runs before any transfer, and `withdraw_treasury` reserves `net` alongside the token refunds |
| `network_fee_lamports` drifts from the real fee (priority fees) | Medium | It is a policy value with a cap; review it with the flat on the same schedule |
| One policy number cannot describe both a one-signature settle (a self-sponsored wallet, an allowance) and a two-signature one (a relayer settlement), so a flat refund would hand back more lamports than the path spent | High | `network_fee_lamports` is the cost of **one signature**. `fee_for_signatures` multiplies it by the settle's own signature count — 2 when `sponsor != payer`, 1 otherwise — and the settlement charges the seller and refunds the sponsor that same multiplied figure. The policy's token equivalents are per-signature for the same reason. Priority fees above the base fee are still the relayer's own cost, which is why the number is a policy value rather than a constant |

## Rollout / Deployment

- [ ] Program: deploy the phase-1 build, then `migrate_config` if the account is
      version 3, then `set_fee_price_anchor` so the token equivalents are priced.
- [ ] Policy: write `network_fee_lamports` in the same `update_config` call, and
      leave it at zero until the client passes a sponsor.
- [ ] API: deploy with the relayer key already configured.
- [ ] Client: deploy after both.
- [ ] Smoke tests: sponsored SOL, sponsored token, unsponsored SOL — on devnet,
      with the payer's lamport delta checked in each.

## Definition Of Done

- [ ] All acceptance criteria are met.
- [ ] `task verify` passes.
- [ ] `README.md` and `DEV.md` describe the SOL rule.
- [ ] The plan is archived and `plans/README.md` is updated.

## Open Questions

- Should an unsponsored SOL payment charge the seller nothing for the network
  (today) or refuse with a named error?
- Does the API need to report the fee payer on a settle it did not build, for
  support and accounting?
