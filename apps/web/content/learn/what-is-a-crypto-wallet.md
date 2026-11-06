---
title: What a crypto wallet actually is
description: A wallet is not a purse. It is a pair of keys that proves the money on the network is yours — which is why nobody can reset it for you.
category: basics
order: 1
---

If you have only ever used a bank, the word "wallet" is misleading. A crypto wallet does not
hold your money in the way a purse holds cash. The money lives on a public network, as a
balance attached to an address. What the wallet holds is the one thing that can move it: the
**private key**.

## The three parts you will meet

- **Your address** — a long string like `8D8GowHJyQRpPqEtAZy4H1DhjpbC…`. It is public. You can
  share it in a message, print it on an invoice, or post it online. People use it to send you
  money.
- **Your private key** — the secret that signs payments for that address. Whoever has it
  controls the balance, and there is no one to call if it leaks.
- **Your recovery phrase** — twelve or twenty-four ordinary words that *are* your private key,
  written in a form humans can back up. More on this in
  [Your recovery phrase is the whole wallet](/learn/protecting-your-seed-phrase).

A wallet app is a keyring plus a window: it stores the keys on your device and shows you what
the network says your address holds.

## Why nobody can reset your password

A bank keeps your balance in its own database, so it can verify your identity and reset
anything you forget. A crypto wallet has no such database and no account. The network only
checks one thing: **was this payment signed by the key that owns this address?** No signature,
no payment — and no support desk, government or company can produce that signature for you.

That trade is the whole point of self-custody: nobody can freeze your balance, and nobody can
undo a mistake on your behalf either. Treat the keys like the deed to a house, not like a
password you can reset.

## What this means day to day

- **Your money is not "in the app".** Delete the app, reinstall it on a new phone, restore the
  recovery phrase, and the same balance appears. The app was never the wallet.
- **Payments are final.** There is no chargeback and no "reverse the transfer" button. Check the
  amount and the recipient before you approve — see
  [Read before you sign](/learn/checking-before-you-sign).
- **The network is public.** Anyone can see a balance and the history of an address. Nobody can
  see your name unless you tie it to the address somewhere, so it is worth thinking of an
  address as a pseudonym, not as anonymity.
- **There are many networks.** An address on Solana is not the same as an address on another
  chain. Sending to the wrong network usually means the money is gone, because no one can move
  it back.

## A safe way to start

Most people learn best with small amounts. A practical order:

1. Install one wallet you trust, and nothing else, from the official app store link on the
   project's own website.
2. Write the recovery phrase down on paper. Do not photograph it.
3. Receive a small amount, send a small amount back to yourself, and see what a fee looks like.
4. Only then raise the amounts, and only once you are comfortable reading the approval screen.

If you are on Payvand's test network, that is exactly what the tokens are for: they are free,
they are worth nothing, and spending them teaches you the same motions you will use later with
real money.
