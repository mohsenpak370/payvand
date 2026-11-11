---
title: What is a blockchain?
description: A shared notebook with thousands of copies, where changing one old line gives you away. This is why a crypto payment cannot be quietly undone.
category: basics
order: 2
---

Forget the word "blockchain" for a moment. Picture a notebook.

Every time somebody pays somebody, a new line is written in the notebook: *this address sent this
much to that address*. Thousands of people keep an identical copy of the same notebook, and each
new page has to be checked by all of them before it counts.

That is a blockchain. "Block" just means a page. "Chain" means each page carries a fingerprint of
the page before it, so the pages are locked together in order.

## Why nobody can cheat the notebook

Each page is stamped with a long code — a **fingerprint** of everything on it. Change one character
on a page, and its fingerprint changes completely. Then the next page's fingerprint no longer
matches it, and neither does the one after that.

So if somebody quietly edits an old line in their copy, every page after it stops adding up. The
other copies disagree, the edit is rejected, and the cheater is the one who looks wrong.

To rewrite history you would have to redo every page since, on more than half the copies at once,
faster than the honest copies keep adding new pages. On a serious network that is not a practical
ambition, it is a fantasy.

## What "on-chain" means

**On-chain** means written in that notebook: visible to everyone, unchangeable afterwards.
**Off-chain** means recorded in somebody's private database — a bank, a shop's till, an app's
server — where they can edit it.

When a shop says your payment is "on-chain", it means the notebook has the line, and the shop is
not relying on anyone's word.

## Why a payment is final

There is no undo button in the notebook, and, more importantly, nobody with the authority to press
one. There is no company to call, no card issuer to file a dispute with, no bank that can pull the
money back.

This cuts both ways. A shop never has to worry about a payment being reversed weeks later — which
is why crypto payments can be cheap instead of carrying insurance costs. And you cannot get your
money back from a wrong address, or from someone who tricked you. The
[safety guides](/learn/checking-before-you-sign) are entirely about that half of the deal.

## Who writes in the notebook

Nobody owns it, so who does the work of adding pages? On Solana, a set of computers called
**validators** take turns. They are paid the small fee attached to each payment, and they have money
of their own locked up as a deposit that they lose if they cheat. The next guide explains the
network itself: [What is Solana?](/learn/what-is-solana)

## Who can read it

Anyone. Balances and payment histories are public — an address, not a name, but all of it visible
to anyone who looks. This is useful (a shop can watch for payment without being told) and worth
remembering (an address used publicly is a very public address).

## What this means for you

A blockchain is a receipt that nobody can fake and nobody can take back. That is worth a great
deal at a shop counter, and it means the moment before you approve a payment is the only moment
anyone can stop it. On Solana that moment is now followed by "done" in well under a second —
[here is what finality means](/learn/when-is-a-payment-final).
