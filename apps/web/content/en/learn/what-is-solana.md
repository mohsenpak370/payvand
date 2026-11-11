---
title: What is Solana?
description: One of the public payment networks — built for a lot of small payments rather than a few big ones. What it does, what it costs, and who is in charge (nobody).
category: solana
order: 1
---

Solana is a **network**, not an app and not a company. It is a set of rules that thousands of
independent computers follow so that they all agree on the same record of who paid whom.

You will never "log in to Solana", the same way you never log in to Visa. You use a wallet or an app
that talks to it — [Payvand](/learn/monthly-allowance-explained) is one such app, and
[tapping your card](/learn/paying-with-a-wallet) is roughly the everyday equivalent.

## What it is built for

Most older crypto networks were designed to settle a few large transfers carefully. Solana was
designed for the other case: a great many small payments, quickly.

- **Blocks arrive continuously**, roughly five times a second.
- **Payments are processed in parallel** rather than one at a time, which is how it keeps up with
  thousands of payments per second.
- **A payment is final in well under a second** — about a seventh of a second typically.
  [That number is worth its own guide](/learn/when-is-a-payment-final), because "final" is the word
  that matters at a shop counter.
- **The fee is 0.000005 SOL**, a fraction of a cent, whatever the size of the payment
  ([why it works that way](/learn/what-is-a-fee)).

## Who keeps it honest

Solana uses **proof of stake**. A few thousand computers, called validators, take turns writing new
pages into the record. To take a turn they must lock up a deposit of SOL. If they cheat or go
offline at the wrong moment, they lose part of that deposit.

That is the entire security model, and it has a pleasant side effect: no mining farms, no warehouse
of graphics cards, no competition burning electricity to guess a number. Compared with proof-of-work
networks, keeping Solana running uses very little energy for the number of payments it handles.

Nobody can switch it off, and nobody can quietly change the rules: changing anything significant
requires the people who run validators, and the holders who delegate to them, to agree.

## What SOL is

SOL is the network's own coin, with two jobs:

- **It pays fees.** Every payment carries a fee in SOL, including payments in dollars.
- **It secures the network.** Validators lock SOL as their deposit, and people who hold SOL can
  delegate it to a validator and earn a share of the rewards.

People also buy and hold SOL as an investment, which is why its price moves around. You do not need
to care about the price to use the network, beyond keeping a little SOL for fees.

## Dollars and euros on Solana

USDC and EURC are tokens issued by Circle that live on Solana and are worth one dollar and one euro
each. A shop can quote you 4.50 EURC and know that is what it will receive a second later —
[the stablecoin guide explains why that works](/learn/stablecoins-explained).

## An honest word about outages

Like any network run by many independent computers, Solana has had bad days: a handful of times in
its history, block production has stalled for a while and payments had to wait. It is rare, it is
public when it happens, and it is one reason apps show you a payment as *confirmed* rather than
assuming it worked.

## What this means for you

Solana is the plumbing that lets a payment at a counter be as quick as a card tap and cheaper than a
card fee, without a company in the middle taking a cut. You do not need to understand the plumbing
to use it — but knowing that nobody owns it explains both the freedom and the responsibility.
