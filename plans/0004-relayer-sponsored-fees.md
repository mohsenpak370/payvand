# 0004 — Relayer and sponsored fees (gasless payments)

Status: `In Progress` (2026-11-22). Follow-up to `0002`, whose out-of-scope note
reads: "Payer-side gasless/relayer support: the payer still needs SOL for the
network fee and the temporary invoice rent on token invoices until a sponsor
exists." This file is where that sponsor is designed.

Increment 1 (2026-11-22): Task A decided and recorded below, Task B shipped in
the program, and the two harness tests of Task E written. `cargo build-sbf`
clean, `cargo test --test settle` 59 passed (the 57 before plus the two
sponsored ones), the IDL rebuilt and copied to
`apps/web/app/assets/payvand.idl.json`, and the web client's token settle now
passes `sponsor: payerAddress` — the funded path, unchanged in cost.

Increment 2 (2026-11-22): Task C shipped as `apps/api/src/sponsor.rs`
(`GET /api/sponsor` plus `POST /api/sponsor/settle`, the byte match, the two
libSQL caps and the `sponsored_settlements` table), and Task D's choice shipped
in the checkout: the buyer's balance decides whether the relayer is asked, the
buyer signs a sponsor-paid message when it is, and a funded wallet still sends
exactly what it sent before. `cargo test sponsor::` 5 passed, web `vitest run`
over the checkout suites and `nuxt typecheck` clean. Left: Task E's devnet run
and the two Task D items this increment did not take (the session-key case, and
a client-side refusal that names the missing lamports).

Increment 3 (2026-11-24): the balance gate is gone. The product's rule is that
the network fee comes out of Payvand's fee, not out of the payer's wallet, so
every token payment now asks the relayer first — a funded wallet no longer
spends its own lamports on a settle, and a payer holding no SOL at all can pay.
Only a payment no relayer will take falls back to the wallet fronting the fee
and rents itself, and a wallet that cannot is refused with
`errors.paymentNeedsSol` before it signs instead of failing inside an approved
wallet prompt. The checkout's cost panel shows the network fee as covered by
Payvand whenever the relayer answers yes, and `profile.detailFeeNote`, `README`
and `DEV` say the same. Tests: `lets the relayer pay even when the wallet
could`, `pays its own fee and rents when no relayer takes the payment`,
`refuses before a signature when there is no relayer and no SOL`. Still open:
the sponsor key on the deployed API (`SPONSOR_KEYPAIR`), Task E's devnet run,
the learn articles and terms copy that still says a payer needs SOL, and gasless
**SOL** invoices, which need the program to take a sponsor on `settle_payment`.
Note also that sponsorship is behind `auth::require_user`, so a buyer who is not
signed in falls back to paying the fee from their own wallet.

## Model (Task A, decided 2026-11-22)

- **In-house sponsor**, not Kora. The sponsor is one keypair the API holds, and
  `apps/api` already has what a paymaster needs around it: sessions (`auth.rs`),
  rate limits (`rate_limit.rs`), a database for metering, and a Vercel
  deployment. Kora would add a second service, its own policy engine and its own
  operator for exactly one instruction; nothing in the program needs an external
  paymaster, and the policy (who may be sponsored, for how much) is the
  product's. Revisit only if sponsorship grows beyond the settle instruction.
- **Token invoices only.** A SOL invoice is paid with the payer's own lamports,
  so sponsoring it means buying the goods. Token invoices are the case where the
  buyer holds the value and the fee is the only obstacle.
- **The sponsor pays the network fee and advances both record rents**, and the
  treasury's refund of those rents goes to the sponsor (Task B). That is the
  whole anti-faucet decision: a sponsor that topped the buyer up would leave it
  holding 3,939,360 lamports a settlement at mainnet's rate.
- **Caps are the API's**: a per-wallet daily sponsored settlement count and a
  daily total, both persisted in libSQL. When the sponsor is empty or a cap is
  reached, the answer is an explicit refusal naming the missing lamports — a
  caller that holds SOL still settles exactly as it does today, without the
  sponsor.
- **The allowance path keeps its float.** A budget's session key still fronts
  and is refunded by the treasury, so nothing changes there; letting it borrow a
  sponsor's fee when the float is drained is deferred, because
  `pay_from_token_budget` would need the same refund-to-sponsor account first.

## What exists today (2026-11-22)

- Every SOL path and every allowance path makes the **payer its own fee payer
  and rent payer**: `SettlePayment.payer` is `Signer` and the `init` payer of
  both the invoice and the receipt (`instructions/sol.rs`). The buyer's wallet
  therefore needs lamports before it can buy anything in SOL, and a token
  invoice used to need them too — the bullet below is what changed that.
- The money flow on a token invoice: the payer (or the session key) advances
  `payment_rent` (the 262-byte `TokenPayment`) and `receipt_rent` (the 48-byte
  marker), the treasury refunds **both** in the same instruction, and the
  invoice account closes to the treasury pool that funded it — never to the
  seller. The seller receives `amount − fee_t`, and `fee_t = max(rate, flat)`
  is where the marker's rent comes back from. A payer's net cost is the price;
  the transient rents only have to exist for the life of the transaction.
- Since increment 1, `settle_token_payment` names a `sponsor` — its last
  account — that advances both record rents and receives the treasury's refund
  of them, while `payer` stays the signer that authorises the buyer's token
  account. A funded wallet passes itself as the sponsor, so one key still signs
  once and its cost is unchanged; a buyer with no SOL can instead pass a
  relayer's key. The two allowance paths are unchanged: the session key fronts
  the rents and the treasury refunds it.
- A SOL invoice is different on both counts: the payer is refunded the marker's
  rent by the treasury, and the seller absorbs it out of the price
  (`fee = max(0, rate − receipt_rent)`), so the seller's total is
  `amount − max(rate, receipt rent)`.
- The allowance paths hide the wallet but not the fee: the session key's float
  (`Config::session_fee_float`, 12,000,000 lamports) pays the network fee and
  advances the rents, and the **treasury pool** refunds both — the token budget
  holds its own rent and nothing more. That float is a **pre-paid sponsor inside
  the buyer's own budget**, funded by the buyer at open time — the reason the
  budget feature already works without a wallet popup.
- `pnpm smoke:silent-payment` and `pnpm smoke:guards` sign and submit from a
  funded key; nothing in the repo submits a transaction whose fee payer is not
  also the payer (or the session key).

## Why it is not just "let a server pay the fee"

1. **The refund is a faucet if the sponsor funds the rents.** On a token
   settlement the treasury refunds the payer `payment_rent` (262 bytes,
   2,714,400 lamports at mainnet's 6,960 a byte) **and** `receipt_rent`
   (48 bytes, 1,224,960) — 3,939,360 together — so a sponsored payer nets
   **+3,939,360 lamports per token settlement** after the 5,000-lamport network
   fee. The treasury recovers the invoice rent when the invoice closes and the
   marker's cost as the seller's flat, but a payer that began with no lamports
   ends the settlement holding both rents — roughly eleven times the 355,680 the
   301-byte receipt leaked. A sponsor that tops the payer up has to fund exactly
   the fee, or the program has to send the refund to the sponsor.
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

- [x] Choose between an **in-house sponsor** (a small service holding a keypair
      that co-signs as fee payer, behind the API's auth and rate limits) and
      **Kora** (an external paymaster with its own policy engine and signer).
      Record which one here, with the reason, before any code. — 2026-11-22:
      in-house; see *Model* above.
- [x] Decide whether the sponsor pays only the network fee, or also fronts the
      transient rents — and if the latter, whether the program sends the
      receipt-rent refund to the sponsor instead of the payer (Task B), so a
      settlement cannot be farmed for lamports. — 2026-11-22: it fronts both
      rents and the whole refund goes to it.
- [x] Decide the caps: per-wallet daily sponsored settlements, a maximum total
      spend, and what happens when the sponsor is empty (the payment should
      still be sendable by a payer who holds SOL). — 2026-11-22: per-wallet and
      global daily counts in the API's database; refusal when empty or capped,
      and the funded path never calls the sponsor.

### Task B: Program — a sponsor the program knows

- [x] Add a `sponsor` account to `settle_token_payment` that is the `init` payer
      and receives the treasury's **whole** refund — the invoice and marker
      rents, 3,939,360 lamports at mainnet's rate — keeping `payer` as the
      signer that authorises the token account. `settle_payment` is untouched:
      Task A refused SOL invoices. — 2026-11-22: done. The account is **required
      and last** rather than optional: Anchor cannot make an `init` payer
      optional, and an account that sometimes exists would be two instruction
      layouts anyway. A funded wallet passes itself, so the semantics are the
      old ones; the transaction gains one duplicate account (the cost in bytes
      is a length byte and 32 bytes of account key) and every caller uses the
      new IDL — `apps/web`, the harness, and any script built from the old one.
- [x] The pre-funding shape is not taken, so nothing is owed here. — 2026-11-22:
      the refund-to-sponsor shape is the anti-faucet proof, and
      `a_sponsored_settle_pays_with_a_wallet_that_holds_no_sol` asserts the
      buyer's zero balance stays zero.
- [x] Keep the receipt's `payer` field meaning "who signed", and decide where
      the sponsor is recorded (a new field, an event field, or not at all). —
      2026-11-22: `PaymentReceipt` is unchanged and `PaymentSettled` keeps its
      shape, because the SOL paths emit that event too and every decoder reads
      it; the sponsor is named in the settle transaction's `msg!` log and is the
      transaction's own fee payer on chain.

### Task C: API — the sponsor endpoint

**Design (2026-11-22).** `POST /api/sponsor/settle` takes the **buyer-signed
transaction**, not a list of instructions: the API rebuilds the one
`settle_token_payment` message it expects from `(buyer, seller, reference, mint,
amount, memo)` — sponsor as fee payer, the derived invoice, receipt, config,
treasury and token accounts — compares the message bytes with the received
transaction's, and only on an exact match adds the sponsor's signature and
submits. A different instruction, account, amount or mint is a byte mismatch and
is never signed, so the sponsor signs exactly one shape instead of trusting a
parser. The builder mirrors `usePaymentProgram`'s IDL-driven one, and a test
pins the two to each other.

- [x] A route that takes an unsigned transaction (or the parameters), checks the
      allowed instruction set and the invoice's price against the chain, signs
      as fee payer, and submits. `apps/api` already has `auth.rs`,
      `rate_limit.rs` and per-user sessions to hang this on. — 2026-11-22:
      `POST /api/sponsor/settle` takes the buyer-signed base64 transaction, and
      `GET /api/sponsor` answers whether a sponsor exists, its address, and the
      lamports a wallet needs before asking for one (`needs_lamports`, the
      262-byte invoice's rent + the 48-byte marker's + one signature =
      3,944,360 at mainnet's rate).
- [x] Reject anything that is not one of the settle/allowance instructions, or
      whose accounts do not match the invoice the API knows — a sponsor that
      signs arbitrary instructions is a wallet, not a sponsor. — 2026-11-22:
      the module rebuilds the single expected `settle_token_payment` message
      from the request's fields and refuses a byte mismatch before signing; it
      never parses the received instruction. The price is the request's, which
      is safe because it is also in the invoice's seeds: a different amount is a
      different invoice, so the message cannot match.
- [x] Meter spend per user and globally, persist it in the API's libSQL
      database (`apps/api/src/db.rs` — this repo does not use D1), and return a
      clear refusal the client can show ("this payment needs SOL for the network
      fee") instead of a silent failure. — 2026-11-22:
      `sponsored_settlements` counts a rolling 24 hours per account
      (`SPONSOR_DAILY_PER_WALLET`, 10) and for the deployment
      (`SPONSOR_DAILY_TOTAL`, 200); the caps are checked before the transaction
      is even decoded, and both refusals are 429s with the reason and the
      remaining-lamports hint in the message. A row is written only for a
      transaction the cluster accepted. `cargo test sponsor::` covers the two
      caps and the keypair parsing.

### Task D: Client

- [x] Send through the sponsor only when the payer's balance is below what the
      transaction needs (fee + rents), so funded wallets keep today's path. —
      2026-11-22: the token wallet path reads the payer's balance and compares it
      with what that settle advances (`invoiceRent + receiptRent + networkFee`,
      which is the endpoint's `needs_lamports` plus the client's fee estimate).
      Below it, `GET /api/sponsor` is asked and, if it answers yes, the
      instruction is built with the sponsor as its fee payer and its `sponsor`
      account, the buyer signs with `useSolanaSignTransactions`, the fee payer's
      slot is left empty, and the base64 wire bytes go to `POST
      /api/sponsor/settle`. Above it nothing is asked of the endpoint at all and
      the same `execute()` send runs as before. *(Superseded 2026-11-24: the
      balance threshold is gone and the relayer is asked for every token
      payment, so the fee is Payvand's cost rather than the payer's — see
      Increment 3. Pinned now by `lets the relayer pay even when the wallet
      could`, `pays its own fee and rents when no relayer takes the payment` and
      `refuses before a signature when there is no relayer and no SOL`.)*
- [ ] Show the sponsored case honestly: the checkout already says the payer
      pays the price and nothing else; confirm no new line appears and that the
      refusal path names the missing lamports. — Half done, 2026-11-22, by
      inspection: no template or `InvoiceCosts` field changed, so the sponsored
      path shows the same price and the same fee note, and the buyer's own
      lamport cost is zero
      by the program's refund to the sponsor (`0002` Task 3.5's rule). A refusal
      from the endpoint arrives as its own message through `failure` — the 429
      text names the daily limit and asks for a little SOL. *Not done:* when
      `GET /api/sponsor` simply reports `available: false`, the client falls back
      to the wallet's own send and the refusal the buyer sees is the chain's,
      not a lamport count (`keeps the wallet’s own path when no relayer is
      available`).
- [ ] Cover the session-key case too: a budget whose float has been drained
      (`0002` Task 2.3's warning) should be able to borrow the sponsor's fee.
      *Deferred by the Model decision above: it needs the same refund-to-sponsor
      account on `pay_from_token_budget`.*

### Task E: Tests and devnet

- [x] Harness: a token settle with a zero-lamport payer and a sponsor as fee
      payer and rent payer, asserting the payer's lamport delta is zero, the
      seller still receives `amount - fee`, the treasury's refund goes to the
      sponsor, and the receipt is written. — 2026-11-22:
      `a_sponsored_settle_pays_with_a_wallet_that_holds_no_sol`; the sponsor ends
      10,000 lamports down, the two signatures' fee and nothing else, and the
      treasury's lamports fall by exactly the marker's rent.
      *(The "second settlement refused" half is already covered for every token
      settle by `settle_token_payment_replay_is_rejected_as_already_settled`, so
      the sponsored test does not repeat it.)*
- [x] Harness: a sponsored settle with a lamport-less payer fails cleanly with
      `TreasuryUnderfunded` when the treasury cannot refund, exactly as an
      unsponsored one does. — 2026-11-22:
      `a_sponsored_settle_is_refused_when_the_treasury_cannot_refund`.
- [ ] Devnet: one sponsored settle through the real endpoint, recorded here with
      its signature and the sponsor's balance change. This is also the first
      time a real wallet signs a transaction whose fee payer is not its own
      account (`useSolanaSignTransactions` over the wire bytes with the fee
      payer's slot empty), which no test here can stand in for.

## Acceptance criteria

- [ ] A buyer holding only USDC (zero lamports) settles a token invoice through
      the sponsor, and the seller, treasury and receipt all match what an
      unsponsored settle produces.
- [ ] The sponsor cannot be induced to pay for anything outside the allowlist,
      and its total spend is bounded by configuration, not by client behaviour.
- [ ] A funded payer's transaction is unchanged (same instructions, same fee
      payer) when the sponsor is not used. *(Amended 2026-11-22: the instruction
      now carries a final `sponsor` account, which a funded payer fills with its
      own key — so the transaction is not byte-identical, but the signer, the
      fee payer, the splits and the buyer's cost are.)*
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
precedent for a third party paying a payer's costs. `0006`'s compressed receipts
would remove the marker and its rent, which re-prices the refund this plan
routes to the sponsor, so do not size the sponsor's cap before that decision.

## Risks And Mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Sponsored refunds become a lamport faucet | High | Fund only the fee, or route the whole rent refund (invoice + marker) to the sponsor (Task B). |
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
