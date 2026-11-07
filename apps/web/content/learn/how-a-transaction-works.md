---
title: What is actually inside a Solana transaction
description: Signatures, a message, account flags, a blockhash with an expiry, a fee in micro-lamports, and a 1,232-byte box. The anatomy, with the real constants.
category: technical
order: 1
level: technical
---

A transaction is not a request to a server. It is a signed object that either executes completely
or changes nothing at all. Here is what is in it, and why each part exists.

## Two halves: signatures and a message

A Solana transaction is `signatures` plus a `message`.

- The **message** contains a header, the list of account addresses it touches, a recent blockhash,
  and one or more compiled instructions.
- The **signatures** are over the serialized message. The first one belongs to the fee payer, and its
  signature is the transaction's ID.

Because signatures cover the whole message, any change to an account list, an instruction, or the
blockhash invalidates them. That is the entire tamper-resistance story at this layer.

## The box it has to fit in

- Legacy and v0 transactions are capped at **1,232 bytes** — the IPv6 minimum MTU (1,280) minus 48
  bytes of headers. Not a design flourish: it is what fits in one network packet.
- A transaction carries at most **12 signatures**.
- It can reference at most **64 accounts**.
- The newer **v1** format raises the cap to **4,096 bytes**.

The byte limit, not the account count, is usually what bites first. A legacy message spends 32 bytes
per account, so a handful of instructions with many accounts overflows 1,232 bytes long before it
reaches 64.

## Versioned messages and lookup tables

**v0** messages can point at an **address lookup table**: an on-chain list of up to 256 public keys.
The message then refers to those accounts by one-byte index instead of carrying each 32-byte key.

The trade-off is deliberate: lookup-table accounts may be writable or read-only but **cannot be
signers**. Signers and the fee payer always stay inline in the message, because a signature has to
cover the key that produced it.

## Instructions and account metas

Each instruction names one program and carries:

- an ordered list of **accounts**, each with `is_signer` and `is_writable` flags, and
- an opaque **data** blob whose layout the program defines.

Programs can invoke other programs during execution; those are **inner instructions**, recorded in
metadata but never serialized as top-level instructions.

The signer/writable flags are not hints. The runtime enforces them: writing to an account that was
not marked writable fails, and using an account as a signer without a matching signature fails.

## The blockhash, and its expiry

Every transaction references a recent blockhash. It does two jobs: it prevents replay, and it gives
the transaction a deadline. A blockhash is usable for **150 slots** (151 ages, given zero-indexing).

At the current **200 ms** slot time that is roughly **30 seconds** — though slot production can skip
or lag, which is why clients track `lastValidBlockHeight` from `getLatestBlockhash` rather than
setting their own timer.

## Fees: a base and a tip

- **Base fee:** 5,000 lamports per signature, so 0.000005 SOL for a typical one-signature payment.
  It is split 50/50 between burning and the validator, and it is charged **even if execution fails**.
- **Priority fee:** set by a compute-unit price in **micro-lamports per CU**, with the total equal to
  `ceil(price × requested CU limit / 1,000,000)`. Note *requested*: you pay for the limit you ask
  for, not the compute you actually use. All of it goes to the validator.

## Compute: the real budget

- A non-builtin instruction gets **200,000 compute units** by default; a builtin gets 3,000.
- The transaction-wide cap is **1,400,000 CU**.
- The scheduler's per-block limit is **60,000,000 CU** — a capacity figure, not a per-transaction
  allowance.

Programs can request a different limit with the compute budget instructions, and that request is
what the priority fee is calculated against.

## Simulation, preflight, and what "sent" means

`sendTransaction` verifies signatures and **simulates** the transaction against a commitment level
before forwarding, unless `skipPreflight` is set. Preflight failing is a strong signal the
transaction would fail on-chain — which is why apps simulate before showing a wallet popup.

But a successful preflight is not a confirmation. `sendTransaction` returns a signature as soon as
an RPC accepts the submission. The transaction is only real once it is in a block and final.

## Commitment after Alpenglow

Since Alpenglow activated on mainnet in **epoch 1064 (21 October 2026)**, `confirmed` and
`finalized` both mean permanently committed; `processed` means executed in a bank but not yet
final. Solana's upgrade notes recommend new code use `finalized`, and say `confirmed` is slated for
removal in a later release. See [the finality explainer](/learn/when-is-a-payment-final) for what
that changes at a counter.

## Sources

- [Transaction structure](https://solana.com/docs/core/transactions/transaction-structure)
- [Versioned transactions](https://solana.com/docs/core/transactions/versioned-transactions)
- [Constants reference](https://solana.com/docs/core/constants-reference)
- [Fee structure](https://solana.com/docs/core/fees/fee-structure) · [Compute budget](https://solana.com/docs/core/fees/compute-budget)
- [sendTransaction](https://solana.com/docs/rpc/http/sendtransaction) · [simulateTransaction](https://solana.com/docs/rpc/http/simulatetransaction)
- [Anza: commitment status](https://docs.anza.xyz/consensus/commitments)
