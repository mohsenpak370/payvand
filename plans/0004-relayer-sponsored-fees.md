# 0004 — Relayer and sponsored fees (gasless payments)

Status: `Draft`. Follow-up to `0002`, whose out-of-scope note reads: "Payer-side
gasless/relayer support: the payer still needs SOL for the network fee and the
temporary invoice rent on token invoices until a sponsor exists." This file is
where that sponsor is designed; it is not approved for implementation.

## What exists today (`0002`, revision 17)

- Every settle path makes the **payer its own fee payer and rent payer**.
  `SettlePayment.payer` and the token settle's `payer` are `Signer` and the
  `init` payer of both the invoice and the receipt (`instructions/sol.rs`,
  `instructions/token.rs`). The buyer's wallet therefore needs lamports before
  it can buy anything, even when the invoice is priced in USDC.
- The money flow on a token invoice: the payer advances `payment_rent`
  (`Payment` 230 bytes) and `receipt_rent` (301 bytes), the treasury refunds
  `receipt_rent` to the payer, and the invoice account closes to the **seller**,
  which is why the seller is paid `amount - payment_rent - fee` at settle time.
  A payer's net cost is the price; the transient rents only have to exist for
  the life of the transaction.
- The allowance paths hide the wallet but not the fee: the session key's float
  (`Config::session_fee_float`, 12,000,000 lamports) pays the network fee and
  advances the rents, and the pot makes it whole. That float is a **pre-paid
  sponsor inside the buyer's own budget**, funded by the buyer at open time —
  the reason the budget feature already works without a wallet popup.
- `pnpm smoke:silent-payment` and `pnpm smoke:guards` sign and submit from a
  funded key; nothing in the repo submits a transaction whose fee payer is not
  also the payer (or the session key).

## Why it is not just "let a server pay the fee"

1. **The refund is a faucet if the sponsor funds the rents.** `receipt_rent`
   (2,179,320 lamports at devnet's 5,080 per byte) goes back to the payer while
   `payment_rent` (1,818,640) comes out of it and goes to the seller, so a
   sponsored payer nets **+355,680 lamports per token settlement** after the
   5,000-lamport network fee. A sponsor that tops the payer up has to fund
   exactly the fee, or the program has to send the refund to the sponsor.
2. **The payer still signs.** `settle_token_payment` proves the payer's
   authority over `payer_token_account` (`token::authority = payer`), so a
   sponsor cannot sign for the buyer. Any design here is a second signature, not
   a substitute for the first.
3. **SOL invoices cannot be gasless at all** without the sponsor paying the
   price, since the payer's own lamports are the payment. The product case is
   token invoices and the allowance paths.
4. **A sponsor is a key worth stealing.** Whoever it is, its lamports buy
   arbitrary preflighted transactions; it needs a spend ceiling, an allowlist of
   instructions, and metering that a compromised client cannot bypass.

## Tasks

### Task A: Decide the model before the code

- [ ] Choose between an **in-house sponsor** (a small service holding a keypair
      that co-signs as fee payer, behind the API's auth and rate limits) and
      **Kora** (an external paymaster with its own policy engine and signer).
      Record which one here, with the reason, before any code.
- [ ] Decide whether the sponsor pays only the network fee, or also fronts the
      transient rents — and if the latter, whether the program sends the
      receipt-rent refund to the sponsor instead of the payer (Task B), so a
      settlement cannot be farmed for lamports.
- [ ] Decide the caps: per-wallet daily sponsored settlements, a maximum total
      spend, and what happens when the sponsor is empty (the payment should
      still be sendable by a payer who holds SOL).

### Task B: Program — a sponsor the program knows

- [ ] Add an optional `sponsor` account to `settle_token_payment` (and to
      `settle_payment` only if Task A admits SOL invoices) that is the `init`
      payer and receives the treasury's receipt-rent refund, keeping `payer` as
      the signer that authorises the token account. No seeds, no account list
      order change for existing callers: the account is the payer when none is
      given, which keeps today's instruction byte-identical in behaviour.
- [ ] Or, if Task A chooses the pre-funding shape instead, prove in a test that
      the sponsored path cannot leave the payer better off than before the
      settlement (the +355,680-lamport case above).
- [ ] Keep the receipt's `payer` field meaning "who signed", and decide where
      the sponsor is recorded (a new field, an event field, or not at all).

### Task C: API — the sponsor endpoint

- [ ] A route that takes an unsigned transaction (or the parameters), checks the
      allowed instruction set and the invoice's price against the chain, signs
      as fee payer, and submits. `apps/api` already has `auth.rs`,
      `rate_limit.rs` and per-user sessions to hang this on.
- [ ] Reject anything that is not one of the settle/allowance instructions, or
      whose accounts do not match the invoice the API knows — a sponsor that
      signs arbitrary instructions is a wallet, not a sponsor.
- [ ] Meter spend per user and globally, persist it in D1, and return a clear
      refusal the client can show ("this payment needs SOL for the network
      fee") instead of a silent failure.

### Task D: Client

- [ ] Send through the sponsor only when the payer's balance is below what the
      transaction needs (fee + rents), so funded wallets keep today's path.
- [ ] Show the sponsored case honestly: the checkout already says the payer
      pays the price and nothing else; confirm no new line appears and that the
      refusal path names the missing lamports.
- [ ] Cover the session-key case too: a budget whose float has been drained
      (`0002` Task 2.3's warning) should be able to borrow the sponsor's fee.

### Task E: Tests and devnet

- [ ] Harness: a token settle with a zero-lamport payer and a sponsor as fee
      payer and rent payer, asserting the payer's lamport delta is zero (or
      zero minus nothing), the seller still receives `amount - fee`, the
      treasury's refund goes to the sponsor, and a second settlement is refused
      by the receipt.
- [ ] Harness: a sponsored settle with a lamport-less payer fails cleanly with
      `TreasuryUnderfunded` when the treasury cannot refund, exactly as an
      unsponsored one does.
- [ ] Devnet: one sponsored settle through the real endpoint, recorded here with
      its signature and the sponsor's balance change.

## Acceptance criteria

- [ ] A buyer holding only USDC (zero lamports) settles a token invoice through
      the sponsor, and the seller, treasury and receipt all match what an
      unsponsored settle produces.
- [ ] The sponsor cannot be induced to pay for anything outside the allowlist,
      and its total spend is bounded by configuration, not by client behaviour.
- [ ] A funded payer's transaction is unchanged (same instructions, same fee
      payer) when the sponsor is not used.
- [ ] `README.md` and `DEV.md` state who pays the network fee in each case.

## Verification

- [ ] `cargo build-sbf --manifest-path programs/payvand/Cargo.toml`, then
      `cargo test --test settle` in `apps/contracts` (never `anchor test` here).
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
- [ ] `anchor idl build`, copied to `apps/web/app/assets/payvand.idl.json`.
- [ ] `apps/api` tests for the endpoint's allowlist, limits and refusal messages.
- [ ] In `apps/web`: `vitest run`, `nuxt typecheck`, `eslint .`.
- [ ] Devnet smoke run recorded here.

## Dependencies

`0002` Task 4.1 (the float sweep), Task 3.2 (float sizing) and revision 12
(pool-funded rents on the budget paths) — the last is the closest existing
precedent for a third party paying a payer's costs.

## Risks And Mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Sponsored refunds become a lamport faucet | High | Fund only the fee, or route the receipt-rent refund to the sponsor (Task B). |
| Sponsor key is compromised | Critical | Instruction allowlist, per-user and global caps, key held in the API's secret store, rotation documented. |
| Sponsor silently fails, payment looks broken | Medium | Explicit refusal the client renders, plus a funded-wallet fallback path. |
| Sponsored flow diverges from the normal one | Medium | One program instruction with an optional account; the funded path keeps its exact account list. |

## Rollout

- [ ] Program: upgrade in place, IDL copied, `Config` untouched.
- [ ] API: deploy with the sponsor key set as a secret; caps in config.
- [ ] Client: sponsor used only below the balance threshold; fallback verified.
- [ ] Smoke tests: one sponsored and one unsponsored settle on devnet.

## Open Questions

- In-house sponsor or Kora — and if Kora, does its policy engine need the
  program to name a sponsor account at all?
- Should a sponsor be able to pay for **SOL** invoices (i.e. buy the coffee), or
  is gasless strictly a token-invoice feature?
- Does the receipt need to record who paid the fee, for accounting and refunds?
- Is there a rate the sponsor must not exceed per seller, to stop a shop from
  farming sponsored settlements against itself?
