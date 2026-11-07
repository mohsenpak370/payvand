---
title: Accounts, ownership and PDAs
description: Everything on Solana is an account with five fields and an owner. Why programs are stateless, why a PDA has no private key, and where a token balance really lives.
category: technical
order: 2
level: technical
---

Solana has no global state object and no per-contract storage. It has **accounts**, and every piece
of data the network remembers is one.

## The five fields

Every account is keyed by a 32-byte address and has the same shape:

| Field | Meaning |
|---|---|
| `lamports` | Balance in lamports (1 SOL = 1,000,000,000) |
| `data` | Arbitrary bytes — program state or program code |
| `owner` | The program allowed to modify `data` or debit `lamports` |
| `executable` | Whether this account holds a program |
| `rent_epoch` | Deprecated; rent-exempt accounts use `u64::MAX` |

Two rules explain most of Solana's design:

1. **Only the owner program may modify an account's data or debit its lamports.** Anyone may *credit*
   lamports to a writable account, which is how a payment works without the recipient's program
   being involved.
2. **An account's size is fixed when it is created.** Growing it means reallocating with enough
   lamports to stay rent-exempt, up to a **10 MiB** per-account maximum.

## Rent, and why there is a deposit

Storing data costs validators real resources, so an account must hold a refundable balance:
`(data_size + 128) × 3,480 lamports × 2 years`. A 165-byte classic token account needs
**2,039,280 lamports** (≈0.00204 SOL); an 82-byte mint needs **1,461,600**.

Ongoing rent collection is disabled; this is now a refundable storage deposit. Close the account and
the lamports come back to you. Query `getMinimumBalanceForRentExemption` for the live number.

## Programs are stateless

A program is an account with `executable = true`, written in a bytecode the runtime can run (sBPF).
It executes only over the accounts an instruction hands it, and it can only change those it owns.

That is why "smart contract storage" does not exist here. A program's memory is the set of accounts
passed in; its persistent state lives in separate **data accounts** that the program owns. When
people say a program is stateless, they mean exactly this.

Programs are deployed and upgraded through a loader program that owns the executable account; the
upgrade authority is a separate key, and losing control of it means losing the ability to fix the
program.

## PDAs, addresses nobody can sign for

A **program-derived address** is computed from `seeds + program_id + bump` and then checked to be
**off the Ed25519 curve**. Up to 16 seeds, each at most 32 bytes.

Off-curve is the point: no private key corresponds to such an address, so no ordinary signature can
ever exist for it. Instead, during a cross-program invocation, the deriving program passes the seeds
back and the runtime verifies that they hash to the PDA, granting it signer privilege **for that
invocation only**. That is `invoke_signed`.

This is how a program controls funds without holding a key. Payvand's allowances work this way: the
pot is a PDA owned by the program, so no person and no server holds a key to it, and spending
requires the program's own rules to be satisfied.

Note the two separate steps people conflate: *deriving* an address is pure arithmetic, and
*creating* an account at it costs lamports and a system-program instruction.

## Where a token balance actually lives

There are three distinct things:

- **A mint account** describes a token: supply, `decimals`, and optional mint and freeze authorities.
  Amounts are stored in base units; decimals only affect display.
- **A token account** holds units of exactly one mint. Its own `owner` field names the authority that
  can move them. The account itself is owned by the Token Program (or Token-2022).
- **A wallet address** is usually just an on-curve keypair account holding SOL. It is not where your
  USDC is.

The **associated token account** (ATA) is the conventional token account for a given
wallet + mint + token-program combination, derived deterministically so anyone can find it. The
Token-2022 program extends this model with optional account and mint extensions, which change account
size and add features such as transfer fees or interest.

## Why write flags matter for throughput

The runtime uses **Sealevel** to execute transactions in parallel. Transactions that only *read* an
account share it freely; transactions that *write* the same account must be serialized. A hot
writable account — one global counter, say — is therefore a throughput bottleneck, while sharded
data accounts are not.

This is a design constraint, not a tuning detail. It is why you will see state split per user, per
mint and per month rather than kept in one large account.

## Sources

- [Accounts](https://solana.com/docs/core/accounts) · [Account structure](https://solana.com/docs/core/accounts/account-structure) · [Modification rules](https://solana.com/docs/core/accounts/modification-rules)
- [Programs](https://solana.com/docs/core/programs) · [Builtin programs](https://solana.com/docs/core/programs/builtin-programs)
- [PDAs](https://solana.com/docs/core/pda) · [PDA derivation](https://solana.com/docs/core/pda/pda-derivation) · [CPI](https://solana.com/docs/core/cpi)
- [Tokens](https://solana.com/docs/tokens) · [Token accounts](https://solana.com/docs/tokens/basics/create-token-account) · [Token-2022 extensions](https://solana.com/docs/tokens/extensions)
- [Anza: read-only accounts and Sealevel](https://docs.anza.xyz/implemented-proposals/readonly-accounts)
