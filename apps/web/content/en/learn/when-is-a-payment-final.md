---
title: When is a payment really done?
description: A card payment can be reversed for weeks. A Solana payment becomes permanent in under a second. What "final" means, and what changed in October 2026.
category: solana
order: 2
---

Here is the everyday version of a technical question. You tap to pay for a coffee. When can the shop
safely hand you the cup and let you walk out?

On a card, the honest answer is "in a few days", which is why card payments can be clawed back and
why shops pay for that risk. On Solana the answer is now **under a second**, and has been since an
upgrade called **Alpenglow** went live on the main network on 21 October 2026.

## Three states a payment can be in

- **Seen (processed).** One computer has the payment in view. It might still be replaced if the
  network reorganises around a different version of events. Not safe to rely on.
- **Confirmed.** Enough of the network has agreed. Before Alpenglow this was fast but weaker than
  final — a payment could be confirmed and, in a rare reorganisation, still lost.
- **Finalized.** Permanently written down. It cannot be undone or reordered, not even by the
  validators who wrote it.

Before Alpenglow, finality took roughly **13 seconds** on Solana. That put shops in an awkward spot:
wait 13 seconds at the counter, or accept "confirmed" and carry a small risk. Neither is how you
want to sell coffee.

## What changed

Alpenglow replaced the old voting system with one where validators vote directly and their votes are
collected into a **certificate**. When validators holding 80% of the staked SOL agree in a single
round, the payment is final immediately; if they do not, a second round settles it.

The practical numbers:

- **Typically about 150 milliseconds** — roughly a seventh of a second — from the payment being
  included to being final.
- Sometimes faster (about 100 ms), sometimes slower. Solana's live dashboard usually shows finality
  around 200 milliseconds.
- It varies with how the stake is distributed, and no guaranteed maximum is published. What you can
  say confidently is that it is **well under one second**.

One consequence worth knowing: under Alpenglow, **"confirmed" and "finalized" mean the same thing**.
The two labels still exist, but both now mean permanently committed. Only "processed" is still an
early, unsafe state.

## Why this matters at a counter

The shop can hand over the coffee the moment the app says confirmed, because there is nothing left
to wait for. No pending limbo, no "we'll see if it clears", no 13-second pause with a queue behind
you. This is what makes a crypto payment feel like a card tap rather than a bank transfer.

## The other side of final

The same mechanism that makes a payment permanently yours to a shop makes a mistake permanently a
mistake. Wrong address, wrong amount, a site that tricked you — once it is final, it is final, and
there is no one to appeal to.

So the speed and the irreversibility are one feature, not two. It is worth reading
[what to check before you sign](/learn/checking-before-you-sign) for exactly that reason.

## What this means for you

When Payvand shows a payment as confirmed, the money has genuinely arrived and stays arrived — in
less time than it takes to put your phone back in your pocket. The part that still deserves your
attention is the half second *before* you approve it.
