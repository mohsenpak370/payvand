---
title: Why every payment costs a fraction of a cent
description: Who gets the fee, why you need a little SOL even when you pay in dollars, and why a failed payment still costs something.
category: basics
order: 6
---

Every payment is a new line written into the shared notebook, and writing it takes a computer doing
real work. The **fee** is what pays for that work.

On Solana the base fee is **5,000 lamports**, which is 0.000005 SOL — a fraction of a cent. It is
charged per signature, so one simple payment is one fee.

## How that compares to a card

With a card, the shop pays roughly 1–3% of every sale to the card network, plus a monthly rental
for the terminal. On a 4.50 payment that is a few cents disappearing from the shop's side, and it is
why small shops sometimes refuse cards under a minimum amount.

A crypto fee is different in two ways:

- **It is flat.** Paying 2 or paying 2,000 costs the same tiny amount, because the work is the same.
- **The payer pays it, and the shop receives the full amount.** There is no terminal rental, no
  monthly contract, and nothing deducted from the sale.

## Why you need a little SOL even to pay in USDC

The fee is always paid in the network's own coin, SOL. Think of it as the postage stamp: the letter
can contain dollars, but the stamp has to be the postal service's own.

That is the one slightly awkward part of getting started: a wallet full of USDC and zero SOL cannot
move anything. Keep a small amount of SOL in the wallet you pay from, and you will never think about
it again.

## When the network is busy

Solana also allows an optional **priority fee** — a small tip for faster inclusion, rather like
paying extra for express post. Most of the time it is not needed, and Payvand does not add one to
ordinary payments.

## Why a failed payment still costs something

If a transaction reaches the network and fails, the changes are undone but the fee is still charged,
because the work was already done. It is a tiny amount, but it is a reason not to mash a button that
keeps failing — fix the reason first.

## One more small cost: the deposit for an account

Creating a new account on the network requires a small deposit of SOL, held while the account
exists. It is **refundable** — closing the account returns it. Payvand's monthly pot does this once,
which is part of why opening an allowance costs slightly more than the limit itself.

## What this means for you

Fees are not something to worry about or budget for. Keep a small amount of SOL in the wallet you
pay from, and the cost of a payment stays so small that it is easier to ignore than to calculate.
