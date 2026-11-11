---
title: Staking, validators and epochs
description: Stake accounts, vote accounts, warmup, commissions and a leader schedule set a day in advance. How Solana's proof of stake actually allocates the right to write blocks.
category: technical
order: 5
level: technical
---

Solana's security budget is staked SOL. Understanding the staking mechanics explains why the network
behaves the way it does under load, and what "validator" really means.

## Two accounts, two jobs

- A **stake account** holds a delegator's SOL and records which validator's **vote account** it is
  delegated to. It is where the economic weight lives.
- A **vote account** belongs to a validator and records its votes and vote credits. It does not hold
  the delegated stake.

One stake account delegates to one validator at a time; splitting across validators means multiple
stake accounts. Delegation is not custody — the validator never gains the ability to move your
stake, only to vote with its weight.

## Warmup, cooldown and epochs

Delegation changes do not take effect instantly. Stake **warms up** to become active and **cools
down** before it can be withdrawn, and the network caps how much can activate or deactivate per
epoch. The exact delay therefore depends on how much stake is moving at once.

An **epoch** is **432,000 slots**. At the current 200 ms slot time that is a nominal **24 hours**;
skipped slots make the real elapsed time vary. Epoch boundaries are when stake weights, leader
schedules, and reward distribution take effect.

Two practical figures:

- **Minimum delegation** is a cluster value; the documented example from
  `getStakeMinimumDelegation` is **1 SOL**. Query the cluster rather than hard-coding it.
- **Commission** is the validator's percentage cut of rewards, set on the vote account. It is charged
  on rewards, never on principal.

## Who produces which block

The **leader schedule** is computed ahead of each epoch. The algorithm seeds a pseudorandom selection
from a PoH tick height, samples the active validator set and its stake, and picks leaders weighted by
stake. The schedule covers 432,000 slots with an 8,192-slot lookahead offset exposed over RPC.

This is why stake concentration matters operationally as well as economically: more stake means more
slots, and a well-run high-stake validator produces a disproportionate share of blocks.

## Rewards, inflation and slashing

Rewards for an epoch are split across participating stake in proportion to
**stake × the validator's vote credits**, minus commission. The published inflation *schedule*
parameters are 8% initial annual inflation, a 15% yearly disinflation, and a 1.5% long-term rate —
these describe the curve, not today's rate.

**Solana does not currently slash delegated stake** for validator misbehaviour. A validator that
goes offline or double-signs loses rewards and reputation, not principal. That is a meaningful
difference from networks where equivocation burns stake, and it is worth stating plainly in any
comparison: security here rests on stake-weighted voting and on delegators moving their stake away,
not on a burn.

Confirming a block requires **two-thirds of active stake**; finality under Alpenglow is expressed
through Votor's certificates (see [the Alpenglow page](/learn/alpenglow-internals)), where the fast
path needs 80% and the fallback paths use 60% thresholds.

## What this means for an application

- **Leader concentration is a latency variable.** A slot whose leader is far from your users, or
  poorly peered, is a slower payment — invisible at 200 ms, visible under stress.
- **Stake moves slowly.** A validator's weight cannot be borrowed for one slot; warmup and cooldown
  make grinding attacks expensive in time as well as capital.
- **Rewards are not free yield.** They are inflation plus fees, shared with a validator that takes a
  commission. Anyone quoting a staking return without naming the commission and the inflation source
  is telling half a story.

## Sources

- [Solana: staking](https://solana.com/docs/references/staking) · [stake accounts](https://solana.com/docs/references/staking/stake-accounts)
- [Anza: stake delegation and rewards](https://docs.anza.xyz/consensus/stake-delegation-and-rewards) · [leader rotation](https://docs.anza.xyz/consensus/leader-rotation)
- [getStakeMinimumDelegation](https://solana.com/docs/rpc/http/getstakeminimumdelegation) · [getEpochSchedule](https://solana.com/docs/rpc/http/getepochschedule)
- [Solana: reduced slot times](https://solana.com/upgrades/reduced-slot-times)
