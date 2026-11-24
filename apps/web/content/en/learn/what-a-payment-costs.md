---
title: What a payment costs, and why
description: Where the money goes when someone pays with Payvand — the network fee, the protocol fee, the permanent record on chain, and the bills that keep the service running.
category: payvand
order: 3
---

Nothing here is free to run, and almost none of what a payment costs is profit. This page lays out
each cost and who pays it, so the number on your screen is never a surprise.

## What the customer pays

Exactly the price on the code — nothing added. No markup, no service charge, no handling fee, and no
network fee on top either: the Solana network fee is already inside the price the seller named, and
so are the two small rents Payvand's relayer fronts to write the payment and its receipt on chain. If
a code says 4 EURC, the customer's wallet shows 4 EURC and the customer's balance falls by exactly
4 EURC.

That holds even for a wallet holding no SOL at all. The relayer pays the transaction's signatures and
advances the record rents, and the same transaction pays it back out of the price, so nothing is left
for the customer to front. Only when no relayer takes a payment does the wallet advance those rents
for the length of its own transaction, and even then the price repays it in the same instruction.

## What the seller pays

Two things, both taken out of the amount rather than added to it, and both shown before a code is
shared:

1. **The protocol fee**, a percentage of the price. It is marginal rather than flat: one rate applies
   to the first slice of a price and a lower rate to everything above a boundary, so a large invoice
   is never charged more per unit than a small one. The boundary differs for SOL and for stablecoins,
   because they are not the same size of money. A ceiling applies as well: one payment never pays more
   than 0.1 SOL, 10 USDC or 10 EURC of it, so a very large invoice pays a fee shaped like a rate
   rather than a share of itself that keeps growing.
2. **The record fee**, on payments made in a token. Every settled payment leaves a small, permanent
   receipt on the blockchain. Space on the blockchain is paid for up front, in SOL, and that space
   can never be reclaimed. The record fee covers that cost, and it is priced from the live SOL rate,
   so it follows what the space costs today rather than what SOL was worth when the rule was written.
   It buys a fixed amount of space, so it is a larger share of a small invoice than of a large one. A
   SOL invoice does not carry it separately, because the seller already hands over that rent out of
   the price.

The record and the network fee together are the floor: the seller's deduction is whichever is larger
of the percentage and that cost, so when a payment is small enough that the percentage comes to less
than the record plus the signature, the percentage is not charged at all — the seller pays the costs
and nothing else, and the protocol takes no fee rather than one too small to cover its own record.

The network fee is counted per signature. A settlement a relayer submits carries two signatures, the
customer's and the relayer's; one that pays its own way carries a single signature. The price is
charged for exactly the signatures that payment used, and the refund to whoever fronted them is the
same figure, so a sponsor never gets back more or less than it spent.

The seller's screen shows the exact amount they will receive, and the fee breaks down into those two
parts, before a customer ever sees the code.

## What it costs us to run this

A payment moves wallet to wallet. We never hold the money, which means we have no float to earn from
and no interest on it. What we do have is a bill:

- **Servers**: the website and the API that builds codes, reads payment status and serves shops.
- **A database**: accounts, shops, products, saved codes and subscriptions.
- **Access to the network**: the RPC service the app reads balances and payment status from, which is
  a paid utility at any real volume.
- **Price information**: exchange rates, so a price in euros can be paid in SOL or dollars at a fair
  rate.
- **Email**: the few messages an account needs — sign-in notices, password resets.
- **Monitoring, backups and security work**: the unglamorous half of running anything that touches
  money.
- **People**: building, fixing and answering questions.

Selling the shops subscription offsets part of that, and so does the protocol fee. The fee is not
priced to maximise what we take: it is priced to keep the service alive while a payment stays cheaper
than a card terminal.

## Why the fee has two parts

A percentage alone cannot cover a fixed cost. The blockchain's receipt costs the same whether a
payment is for one unit or a thousand, so a percentage-only fee would be too small to cover it on
small payments and an unnecessarily large cut on large ones. Splitting the fee — a percentage for the
work, a fixed record fee for the space, and the record as the floor under both — keeps small payments
possible and large ones cheap.

## What can change

The rates and the record fee are set by the platform and follow the cost of running the service; the
record fee is repriced from the live SOL rate, so a move in SOL is followed rather than absorbed. The
price on a code never changes, but the fee is the one in force when the payment is made, so a rate
change also applies to codes already shared and not yet paid. The app shows the split at the rates in
force when you open it, so you are never guessing.
