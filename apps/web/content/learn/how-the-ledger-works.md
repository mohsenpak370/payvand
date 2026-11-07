---
title: How the ledger is written and shared
description: Slots, shreds, Merkle roots, erasure coding, and a mempool that does not exist. The machinery between "signed" and "everyone agrees".
category: technical
order: 3
level: technical
---

A signed transaction is only a claim until a block producer includes it. This page is about the
plumbing that turns claims into a shared, verifiable history.

## Slots and blocks

Time on Solana is divided into **slots** of **200 ms**, each with a designated leader, and leaders
are scheduled a full epoch ahead. A slot is an opportunity to produce a block, not a guarantee: slots
can be skipped, and a leader can produce more than one candidate block.

The practical consequence is that "the current block" is fuzzy for a few hundred milliseconds.
Transactions do not queue in a mempool waiting for inclusion — there is no mempool. They are
forwarded to the leader expected to produce the next slot and either land in that block or expire.

## Why blocks travel as shreds

A block is too large to hand to every validator directly. The leader **shreds** it:

- Data shreds carry fragments of serialized entries, capped around 1,280 bytes each.
- Coding shreds are generated with **Reed-Solomon erasure coding** over groups of data shreds, known
  as FEC sets. A set may hold up to 67 data shreds; 32 is the recommended configuration.
- All shreds are signed by the block producer.
- Each FEC set is merkelized: the leaves are hashed pairwise up to a **Merkle root**, and the leader
  signs that root. Because a shred carries a proof path, any node can verify a piece without holding
  the whole block, and missing pieces can be reconstructed from the coding shreds.

## Turbine, the broadcast tree

Turbine distributes those shreds down a stake-weighted tree rather than to everyone at once. The
leader broadcasts to a root node, which fans out to layer 1, whose nodes fan out to layer 2, and so
on. The root rotates **every shred**, and the peer list is reshuffled per shred to make targeted
attacks harder.

Two other roles are easy to confuse with it:

- **Gossip** is the control plane: peer discovery, contact information, and validator metadata. It
  does not carry transactions.
- **Gulf Stream** is the inbound path: clients and validators forward transactions to the upcoming
  leader they already know about, which is why Solana needs no mempool and why a client's
  blockhash must be recent.

## Proof of History, after Alpenglow

PoH was a verifiable delay: a chain of hashes that let anyone check the order and passage of time
without trusting a clock. For years it also **paced** block production.

That pacing role is gone. Since Alpenglow, Votor's timeouts and certificates decide when blocks
finalize, and the tick entries that carried PoH pacing have left the shred stream. PoH's hashing
construction still exists in the ledger's history, but it is no longer what makes the network tick.

## Forks, briefly

Because slots can be skipped and leaders can produce competing blocks, the network can temporarily
hold more than one view of recent history. Validators converge on one branch, and — since Alpenglow
— finalization is an explicit certificate rather than the accumulated lockouts of TowerBFT.

What matters for an app: a transaction seen in a block is *processed*, which is not final. Only a
finalized block is irreversible. On modern Solana that distinction resolves in about 150 ms, but it
is still the difference between "probably" and "certainly".

## What this explains

- **Why there is a 1,232-byte transaction limit**: it is a network packet, and a block is a stream of
  packets.
- **Why a payment occasionally fails with a blockhash error**: the transaction outlived its ~150-slot
  validity window before anyone included it.
- **Why finality is a consensus property and not a network one**: propagation is how a block spreads,
  consensus is how it becomes permanent. They are separate systems by design, which is precisely what
  let Votor replace consensus without touching how blocks travel.

## Sources

- [Anza: Turbine block propagation](https://docs.anza.xyz/consensus/turbine-block-propagation)
- [Shred specification](https://raw.githubusercontent.com/solana-foundation/specs/main/p2p/shred.md)
- [Solana: reduced slot times](https://solana.com/upgrades/reduced-slot-times)
- [Alpenglow: PoH and tick entries](https://solana.com/upgrades/alpenglow)
- [Gulf Stream](https://solana.com/news/8-innovations-that-make-solana-the-first-web-scale-blockchain)
