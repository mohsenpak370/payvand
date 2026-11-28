# Who pays for a payment

This is the reference for where every lamport and token unit goes when an
invoice is paid, and the rules the program, the API and the web app must keep.
Read it before changing anything in a settle path, the relayer
(`apps/api/src/sponsor*`), the checkout (`apps/web/app/composables/useCheckoutPayment.ts`)
or the fee quote (`apps/web/app/utils/invoiceCost.ts`).

It has been broken once already. Plan 0004 (increment 3, 2026-11-24) made the
relayer pay for every payment. Commit `eba624a` (2026-11-27) then let a SOL
wallet with enough SOL pay its own way, and an uncommitted follow-up did the
same for tokens. Each step looked like a small UX fix, and each one moved a cost
onto the buyer. The rules below are what that change broke.

## The rule

1. **The buyer pays the price and nothing else**, in SOL, USDC or EURC. No fee,
   no rent and no network fee is added to it, and the buyer needs no SOL in the
   wallet to pay a token invoice.
2. **The platform pays the costs of the payment**: the network fee, and the rent
   of every account the settlement opens. It fronts them inside the settle
   transaction.
3. **The seller repays the costs that stay**, out of what they receive: the
   network fee, and the rent of the account that is never closed (the 48-byte
   payment receipt, the replay guard). The platform's margin, if the percentage
   is larger than those costs, comes out of the same deduction.
4. **Rent of an account that is closed is never charged to anyone.** The
   invoice account is opened and closed in the same transaction. Its rent comes
   back, so the seller's proceeds are not reduced by it.

So the seller's deduction is

```
fee = min(cap, max(rate × amount, permanent rent + network fee × signatures))
```

with `rate` the tiered percentage (`fee_bps_below` / `fee_bps_above` around the
currency's tier), `cap` 0.1 SOL or 10 USDC/EURC, and the seller receiving
`amount − fee`. A payment too small for the percentage to cover the costs is
charged exactly the costs: the platform neither loses on it nor earns a margin.

## Who is "the platform"

Two accounts, with different jobs:

- **The relayer** (`SPONSOR_KEYPAIR`, the API's hot key) is the platform's
  float. It is the fee payer and the program's `sponsor` on every wallet
  payment. It fronts the network fee and the two record rents, and the same
  transaction repays all of them, so its balance ends where it started.
- **The treasury** (the `treasury` PDA, plus one token account per mint) holds
  the platform's revenue. On a token payment it also refunds the relayer's SOL
  advances, and it receives the equivalent in the token.

## The money, per path

`invoice rent` and `receipt rent` are `Rent::minimum_balance` of the invoice and
receipt accounts, read from the cluster at settle time. `network` is
`Config.policy.network_fee_lamports` (5 000 on devnet: one signature's base
fee), and `network_t` is its token equivalent
(`network_fee_usdc_base_units` / `network_fee_eurc_base_units`, written by
`refresh_fee_price` from the same SOL price as the token flats). `flat` is the
receipt rent in the token at that price (`flat_usdc_base_units` /
`flat_eurc_base_units`). A relayer-paid settle carries **2 signatures**: the
relayer's and the buyer's.

### SOL invoice, wallet payment (`settle_payment`)

| Who | Moves | Net |
| --- | --- | --- |
| Buyer | sends `amount` | **− amount** |
| Relayer | fronts invoice rent, receipt rent and the 2-signature fee; is repaid `invoice rent + receipt rent + network × 2` out of the price | 0 |
| Seller | receives `amount − invoice rent − receipt rent − fee`, then the invoice rent when the invoice account closes to them | **amount − receipt rent − fee** |
| Treasury | receives `fee − network × 2` | the margin |
| Receipt | keeps its rent forever | — |

The SOL `fee` is `max(rate − receipt rent, network × 2)`, so the seller's total
deduction is `receipt rent + fee`, which is the formula above. The invoice rent
passes through the seller and back, so it costs them nothing. It does make a SOL
invoice's minimum `invoice rent + receipt rent + fee`, because the price has to
carry it for the length of the instruction.

### Token invoice, wallet payment (`settle_token_payment`)

| Who | Moves | Net |
| --- | --- | --- |
| Buyer | sends `amount` tokens | **− amount** tokens, 0 SOL |
| Relayer | fronts invoice rent, receipt rent and the 2-signature fee; the treasury refunds `invoice rent + receipt rent + network × 2` | 0 |
| Seller | receives `amount − fee_t` tokens | **amount − fee_t** |
| Treasury (SOL) | pays the refund; receives the invoice rent back when the invoice closes | − (receipt rent + network × 2) |
| Treasury (token) | receives `fee_t` | + fee_t, which repays the SOL above and carries the margin |

`fee_t = max(rate × amount, flat + network_t × 2)`, so the SOL the treasury
spends on the permanent receipt and the network fee comes back to it in the
token. A seller is never charged SOL, and needs none to be paid in a token.

### Self-paid fallback (no relayer available)

When the relayer cannot pay (no `SPONSOR_KEYPAIR`, or a daily cap reached), the
wallet is its own sponsor and the only signer. The program refunds it the two
rents and `network × 1` in the same instruction, and the seller is charged one
signature's network fee instead of two. The buyer still ends at exactly
`− amount`, provided the transaction pays only the base fee. That is why the
fallback always carries a compute-unit limit: Phantom adds a priority price to a
transaction that names no compute budget, and nothing refunds that price. The
wallet does need enough SOL to front the rents for the length of the
transaction. Without it, the checkout refuses with `errors.paymentNeedsSol`
before any signature.

This path exists so a payment is not lost when the relayer is down or capped.
It must never be chosen while a relayer is available.

### Allowance payments (session key)

`pay_from_budget` / `pay_from_token_budget` are signed by the buyer's session
key alone (1 signature). The key fronts the rents and the fee out of its float
and is refunded in the same instruction (from the price on SOL, from the
treasury on tokens). The seller is charged the same costs at one signature. The
pot pays exactly the price. Opening an allowance is the buyer's own deposit (the
budget account's rent, the token vault's rent and the session key's float), and
it is returned when the allowance closes. It is not a payment cost.

### Solana Pay (a code scanned by an external wallet)

`POST /api/solana-pay/{token}` returns the SOL settle already signed by the
relayer as fee payer (`sponsor::solana_pay_settle`), and the wallet signs and
sends it. The money is the SOL table above. When no relayer is available, the
wallet gets the self-paid fallback instead (`sponsor::self_paid_solana_pay_settle`).

## The transaction flow: the relayer signs first

```
page                         API                              wallet
 │ GET /api/sponsor ───────▶ available? relayer address
 │ POST /api/sponsor/prepare ▶ build [limit, settle], relayer = fee payer
 │                             and sponsor; sign; record the signature
 │ ◀─────────────────────── transaction (buyer slot empty)
 │ preparedSettleMatches(): fee payer, limit, settle = the page's own build
 │ signTransactions ────────────────────────────────────────▶ simulate, preview, sign
 │ POST /api/sponsor/settle ▶ bytes == what it signed? both signatures valid?
 │                             was the signature recorded? → send
 │ ◀─────────────────────── signature
```

Why this order: a wallet asked to sign a transaction whose fee payer has not
signed yet treats it as incomplete and shows a warning instead of the price.
With the relayer's signature already on it, the wallet is handed a transaction
that only lacks its own signature, which is the normal multi-signer case. It can
simulate that transaction and show the buyer "− 1.50 USDC".

The page does not trust the API's bytes blindly. `preparedSettleMatches`
(`usePaymentProgram.ts`) checks that the relayer is the fee payer, that there is
no lookup table, and that the message is exactly a compute-unit limit plus the
page's own `settle` (same program, data, and accounts in order). If anything
differs, the payment is refused with `errors.preparedMismatch` and no wallet
prompt opens.

The API does not trust the returned bytes either. `check_prepared`
(`sponsor/handler.rs`) requires the message to be byte for byte the one it would
build for the request's fields, the relayer's signature to be the one over
those bytes, the signature to have been recorded by `prepare`, and the buyer's
signature to verify. A wallet that changed the transaction (for example by
adding a priority fee) has also broken the relayer's signature, so the
transaction is refused before it reaches the cluster.

## Caps and what the relayer risks

A signed transaction cannot be altered, so the relayer cannot be made to pay for
anything but this settle. And a settle that lands refunds it in full. The only
real loss is the base fee of a relayer-signed transaction that fails on chain
(about 10 000 lamports), because a prepared transaction can be sent to the
cluster without going through `settle`. Two caps bound that, both over a rolling
24 hours in `sponsored_settlements`:

- `SPONSOR_DAILY_TOTAL` (default 200) counts **every signature `prepare` or
  Solana Pay handed out**. This is the bound on the relayer's exposure.
- `SPONSOR_DAILY_PER_WALLET` (default 10) counts **only settles this API
  submitted** for the wallet. A prepare names a wallet without proving it holds
  it, so counting prepares here would let a stranger use up somebody else's
  allowance.

`POST /api/sponsor/prepare` and `/settle` are also throttled per caller
(`throttle.rs`), and `settle` per wallet (6 a minute).

## Invariants and the tests that pin them

| # | Invariant | Enforced in | Pinned by |
| --- | --- | --- | --- |
| 1 | While a relayer is available, it is the fee payer and sponsor of every wallet payment, SOL or token, however much SOL the wallet holds | `settleThroughRelayer` (`useCheckoutPayment.ts`) | `the platform pays the settle` in `usePaymentCheckout.test.ts` |
| 2 | The buyer's wallet only ever signs a relayer settle the page verified | `preparedSettleMatches` | `preparedSettleMatches` in `usePaymentProgram.test.ts`; `never opens the wallet on a prepared transaction that is not this invoice` |
| 3 | The relayer signs exactly `[compute-unit limit, settle]`, never a priority price | `expected_message` / `legacy_message` (`sponsor/message.rs`) | `the_relayer_signs_a_limit_and_never_a_priority_price` |
| 4 | The relayer sends only the transaction it prepared, fully and validly signed | `check_prepared` | `sponsor::handler::tests` |
| 5 | Every relayer signature is recorded before it leaves the API, and the deployment cap counts it | `sign_and_record`, `store.rs` | `the_global_cap_counts_signatures_and_a_wallet_cap_counts_submissions` |
| 6 | The self-paid fallback names a compute-unit limit, so the buyer's net stays `− amount` | `settleThroughRelayer`, `self_paid_solana_pay_settle` | `lets a funded wallet pay its own way behind a compute-unit limit`; `the_self_paid_solana_pay_settle_is_the_programs_settle` |
| 7 | The sponsor is refunded both rents and `network × signatures` in the same instruction, and the refund goes to the sponsor, never to the payer | `settle_split`, `token_split` | `apps/contracts/programs/payvand/tests/settle.rs` |
| 8 | The seller is charged the permanent receipt's rent and the network fee (or the rate, if larger), never the invoice account's rent | `fee_for_signatures`, the two splits | `tests/settle.rs`; `invoiceCost.test.ts` |
| 9 | Quotes assume the relayer's 2 signatures unless the path is known to carry 1 | `estimateInvoiceCosts` (default `2`), `useCheckoutCosts` | `useInvoiceCosts.test.ts` |
| 10 | `network_fee_lamports` is at least one signature's base fee (5 000), so a refund covers what the fee payer paid | the policy on chain (`pnpm fee:anchor`) | `DEV.md` devnet runs |

## Before you change this flow

- A change that lets the wallet pay while a relayer is available breaks rule 2
  for SOL and the "no SOL needed" promise for tokens. Don't make it to fix a
  wallet preview. Fix the transaction the wallet is handed instead.
- Anything the relayer signs must be built by `expected_message`. Adding an
  instruction means changing the builder, the page's `preparedSettleMatches`, and
  invariant 3's test together.
- A new settle path needs a row in "The money, per path" and in the invariants
  table before it ships.
- If `network_fee_lamports` is ever set below the base fee, or a path starts
  paying a priority fee, the fee payer is no longer made whole. That is a cost
  for whoever pays the fee, the buyer included on the fallback path.

## Not yet verified or still open

- **Wallet preview on devnet.** This flow rests on wallets previewing a
  transaction the relayer has already signed, and leaving it unchanged. That
  is the expected wallet behaviour, but it has not yet been run against
  Phantom with the deployed API. Do it before relying on it, and record the
  result in `DEV.md`.
- A SOL payment leaves the buyer's wallet with `balance − amount`. If that is
  above zero but below the rent-exempt minimum (about 0.00089 SOL), the cluster
  refuses the transaction. The checkout does not check for this yet.
- `withdraw_treasury`'s reserve is sized for a one-signature refund. A
  relayer-paid token refund is one signature larger. The difference is 5 000
  lamports per settlement, inside the reserve's margin.
- The profile's transaction detail shows the fee the program logged
  (`PaymentSettled.fee`, kept in `settled_payments.fee`). A settlement stored
  before that column existed has no logged fee, and is shown as an estimate at
  2 signatures, marked "≈". That estimate is one signature too high if the
  payment went through the fallback or an allowance.
