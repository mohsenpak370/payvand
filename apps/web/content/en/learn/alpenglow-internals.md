---
title: Alpenglow, from the inside
description: Votor's certificates, the 80% fast path, the 60% fallback, and what "final in 150 ms" costs in assumptions. The consensus change that replaced TowerBFT.
category: technical
order: 4
level: technical
---

Alpenglow is the largest change to Solana's consensus since launch: it replaced **TowerBFT** with a
new finalization mechanism called **Votor**. It is live on mainnet, activated at the start of
**epoch 1064 on 21 October 2026** after SIMD-0326 passed with 98.27% of participating stake in
September 2025.

Its second phase, **Rotor**, which replaces the Turbine block-propagation protocol, is not shipped
and not scheduled. Do not conflate the two — the upgrade page is explicit about the order.

## What TowerBFT did, and why it had to go

TowerBFT reached finality by accumulating **lockouts**: each vote on a later block increased a
validator's lockout, and finality required roughly **12.8 seconds** of accumulated confirmations.
Votes themselves were transactions, included in blocks — consensus traffic competed with user
payments for the same block space.

That is survivable for settlement. It is hopeless for a coffee queue.

## How Votor finalizes

Validators now exchange votes **directly**, outside the block stream, and aggregate them into
**quorum certificates** — stake-weighted proofs that a threshold voted a particular way. SIMD-0326
describes five certificate types:

| Certificate | Threshold | Role |
|---|---|---|
| Notarization | 60% | Validators agree on a block |
| Skip | 60% | Validators agree to skip a slot |
| Finalization | 60% | Second-round finalization |
| Fast-Finalization | 80% | Finality in one round |
| Notar-fallback | 60% | Path when the fast round does not fill |

The happy path is therefore: if **80% of stake** notarizes a block in a single round, a
Fast-Finalization certificate exists and the block is final — no second round, no lockout
accumulation.

If that does not happen, the protocol falls back to a slower two-round path using the 60% thresholds:
a notarization certificate plus the appropriate second-round certificate. Finality still arrives,
just with one more round trip.

## The fault model

Alpenglow is designed for **20% adversarial stake plus 20% offline stake**, combined. It is not a
claim that 40% Byzantine stake is tolerated — the 20+20 split is the assumption, and it is what the
fast path's 80% threshold encodes.

That matters when reading marketing numbers. "80% of stake" is not a supermajority in the classical
two-thirds sense; it is the threshold that lets one round be sufficient *given* the fault model.

## Where 150 ms comes from

Anza's design post cites a **median of about 150 ms**, with cases around **100 ms**, from simulations
using the then-current stake distribution, explicitly excluding computation overhead. Solana's
upgrade page frames ~150 ms as the expectation, and the live dashboard typically shows around 200 ms.

There is no published maximum. The honest engineering statement is: one round trip between
validators, plus transmission, plus variance in stake distribution and slot timing.

## What changed beyond the number

- **Vote transactions are gone from blocks**, so consensus no longer consumes block space that
  payments need.
- **Proof of History stopped pacing block production.** Votor's timeouts and certificates took over
  that job, and tick entries left the shred stream.
- **`confirmed` and `finalized` converged.** Anza documents both as permanently committed;
  `processed` remains the weak early state. New code should prefer `finalized`, and Solana says
  `confirmed` will be removed in a later release.
- **Execution is untouched.** The SVM, programs, account model and transaction format did not change.
  A payment app does not need to be rewritten; it needs to trust a different commitment label.

## How to check a cluster

`getAgGenesisCert` reports whether a cluster has migrated, and the Alpenglow genesis for mainnet is
slot **459,653,000**. On devnet, activation was epoch **1167, 25 September 2026**. For anything
user-facing, prefer querying commitment rather than assuming a date.

## What it means for an app like this one

A point-of-sale flow can treat "finalized" as "hand over the goods", because there is no lockout
period left to wait out. The risk it does not remove is the human one: finality makes a wrong
address wrong forever, faster than ever. That is why the approval screen still matters more than the
confirmation badge.

## Sources

- [Solana: Alpenglow upgrade and status](https://solana.com/upgrades/alpenglow) · [dashboard](https://solana.com/upgrades/alpenglow/dashboard)
- [SIMD-0326: Alpenglow](https://github.com/solana-foundation/solana-improvement-documents/blob/main/proposals/0326-alpenglow.md)
- [Anza: Alpenglow, a new consensus for Solana](https://www.anza.xyz/blog/alpenglow-a-new-consensus-for-solana)
- [Anza: commitment status](https://docs.anza.xyz/consensus/commitments)
