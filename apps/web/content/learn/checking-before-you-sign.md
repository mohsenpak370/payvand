---
title: Read before you sign
description: The approval screen is the last moment anyone can stop a payment. What to check, and the difference between sending money and letting a program spend it later.
category: safety
order: 3
---

Every payment in crypto passes through one moment: a screen in your wallet asking you to sign.
Up to that point, nothing has moved and anything can be cancelled for free. After it, the payment
is final. Treat that screen as the real decision, not a formality to click through.

## What a transaction actually contains

A transaction is a list of instructions plus a fee. For a simple payment the list is short: move
this amount of this token from my address to that address. Your wallet turns that into something
your key can sign, and the network executes it.

Read four things every time:

1. **The amount** — including decimals. `0.5` and `5` look similar at a glance.
2. **The currency or token** — a token can be named anything. If an app quotes USDC but the wallet
   shows an unfamiliar token, something is wrong.
3. **The recipient** — check the beginning and end of the address, and more importantly, that it
   is who you expect. If you are paying a shop, it should be the shop's address from the invoice
   you just scanned, not one you pasted from a chat.
4. **The fee** — small on Solana, but a wildly high fee is a signal that the transaction is not
   what you think.

## Sending is not the same as approving

There are two very different things a signature can do:

- **A transfer** moves a specific amount, once, to a specific address. This is what a payment is.
- **An approval** gives a program permission to move tokens from your account later, sometimes up
  to an amount far larger than the payment in front of you. It is a standing permission, not a
  payment.

Approvals exist for good reasons — swapping tokens needs one — but they are also the favourite
tool of drainers, because a single approval can empty a balance long after you closed the tab. If
you did not expect an approval, cancel it. If you must grant one, grant a specific small amount
rather than an unlimited one, and revoke it afterwards.

## Why the amount can be trusted here

Payvand's invoices put the amount and the recipient *inside the address the payment must reach*.
There is no separate "amount" field a server could change between the QR code and your approval:
pay a different amount and the payment would go to a different address, which is not the one the
seller is watching. So the seller cannot raise the price after you scan, and a page cannot quietly
redirect the funds.

When you do pay, approve exactly what the invoice shows. If the wallet popup disagrees with the
screen you scanned, stop and ask — one of the two is not what you think it is.

## Habits worth keeping

- **Sign one thing at a time.** A queue of approvals is designed to make you stop reading.
- **Do not sign on a device you do not control.** Shared computers and remote-support sessions are
  how "helpful" strangers empty wallets.
- **Prefer a hardware wallet for large amounts.** It shows the amount and recipient on its own
  screen, so a compromised computer cannot swap them behind your back.
- **If in doubt, reject.** Rejecting costs nothing and the payment can be retried in a minute.
