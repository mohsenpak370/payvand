---
title: What happens when you pay with a wallet
description: From scanning the code to the money arriving — the six steps every crypto payment goes through, and what you are actually approving.
category: basics
order: 2
---

A crypto payment feels like a card payment because the shop's screen behaves the same way. Under
the hood it is much shorter: there is no card network, no acquirer and no settlement batch. Here
is the whole path, in the order it happens.

## 1. You get a payment request

In Payvand, the seller enters an amount and shows you a QR code. The code is not an image of a
payment — it is the payment itself: the seller's address, the amount, the currency, and a short
reference that keeps this invoice distinct from every other invoice for the same seller. When you
scan it, your wallet reads those details directly.

Nothing about the seller's request has been "sent to a server" that could change the amount
afterwards. The amount is baked into the address the money must reach.

## 2. Your wallet shows you what you are about to approve

The wallet popup lists the payment. This is the single most important screen in crypto, so read
it: **amount, currency, recipient, network fee**. If the screen says something different from the
seller's screen, stop and check why before signing. The habit is covered in
[Read before you sign](/learn/checking-before-you-sign).

## 3. You approve, and the wallet signs

Approving does not "tell the app to pay". It unlocks the private key on your device just long
enough to sign this one transaction, then locks it again. The app you are using never sees the
key; it receives a signature, which is only valid for that exact transaction.

## 4. The transaction goes to the network

The signed transaction is sent to the network and included in a block. On Solana this normally
takes a couple of seconds. You can watch it happen from the confirmation link the wallet or app
shows.

## 5. The seller's side updates

Because the network is public, the seller's page can watch the seller's address and see the
payment land without anyone telling it. In Payvand the invoice flips to **Paid** at that moment —
the same second you see your wallet confirm.

## 6. The fee

Every transaction pays a small network fee. On Solana the base fee is 5,000 lamports — that is
0.000005 SOL, a fraction of a cent — plus an optional priority fee when the network is busy. The
fee goes to the network, not to Payvand and not to the seller.

You do need a little SOL in the wallet to pay it. A wallet with a large stablecoin balance and
zero SOL cannot move anything, which surprises people the first time.

## What "Paid" is not

- **It is not reversible.** There is no chargeback. Once the network confirms it, the transfer
  stands, so the approval screen deserves your attention.
- **It is not instant everywhere.** Solana confirms in seconds, but other networks can take
  minutes, and a payment is only final once it is confirmed.
- **It is not anonymous.** The transfer is visible to anyone who knows the two addresses.

## The Payvand shortcut: a monthly allowance

Approving every payment gets tiring at a café. Payvand lets you approve **one** transaction that
sets aside a monthly budget in a token such as USDC, and registers a session key held by the app
on your device. After that, payments that fit inside the budget are signed by the session key, so
there is no wallet popup at the counter.

What that key can do is deliberately narrow: it can pay invoices up to the budget you set, this
month, and nothing else. It cannot send funds anywhere of its choosing, cannot exceed the limit,
and you can close the allowance at any time to pull the remaining balance back to your wallet. If
that trade-off does not appeal, pay from your wallet each time — both paths pay the same invoices.
