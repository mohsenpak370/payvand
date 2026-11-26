---
title: What Payvand adds to a wallet
description: A wallet can already show a QR code and send USDC. What it leaves to you is the checking — fake coins with real names, requests that ask for more than a payment, and knowing which sale a transfer was for.
category: payvand
order: 0
---

A wallet such as Phantom or Solflare can already send USDC, EURC or SOL, and most can show a QR
code with an amount in it. So "it makes QR codes" is not the reason to use Payvand. The money even
moves the same way: straight from the buyer's wallet to the seller's, never through us.

The difference is everything around the transfer. A wallet moves money and leaves the checking to
the two people using it. Payvand does that checking for them. Here is what that means in practice.

## Only the real coin counts

Anyone can create a new coin in a few minutes, call it "USDC" and give it the same logo. In a
wallet it looks exactly like the real one. It is worth nothing.

The only reliable way to tell them apart is the coin's **mint address**: a 44-character string of
letters and numbers that identifies the coin, like a serial number. The real USDC on Solana starts
with `EPjFWdd5…`, and a fake one can start with anything. To be sure you were paid in real money,
you would have to open the transfer and compare that string, character by character, every time.
Almost nobody does, and the scam works because of that.

With Payvand you never have to. The real USDC and EURC addresses, from Circle's own published list,
are built into Payvand, and the coin is part of the address the payment has to reach. A payment
in a look-alike coin goes to a different address, one this code is not watching, so the invoice
stays **unpaid**. If it says Paid, the real coin arrived. More on how coins like USDC work in
[stablecoins, in plain words](/learn/stablecoins-explained).

## The customer approves one plain payment

Every crypto payment ends with a screen in the wallet asking the customer to approve. Usually that
is fine. But a signature can do more than send money. It can also give a program **standing
permission** to move funds later, and that is how "wallet drainers" work: a request that looks
routine quietly lets someone else empty the wallet afterwards.

With a plain wallet transfer, the customer also has to type or paste the address and the amount
themselves, and one wrong character sends the money somewhere else for good.

A Payvand request is only ever one thing: **pay this amount, in this coin, to this seller**. The
amount, the coin and the recipient are already filled in from the code, so there is nothing to
type, paste or misread, and there is no extra permission hidden in it. What the customer sees in
the wallet is what happens.

Two honest limits:

- Payvand makes its own request simple and clear. It cannot stop a customer from signing something
  harmful on another site, so the habit in [read before you sign](/learn/checking-before-you-sign)
  still matters, and so do the warning signs in [spotting crypto scams](/learn/spotting-crypto-scams).
- The optional [monthly allowance](/learn/monthly-allowance-explained) is a permission, on purpose.
  It is one the customer chooses to open, with a limit they set, that can only pay invoices and can
  be closed at any time.

## The price and the currency cannot change

The seller sets the price, currency and a short note once, when they make the code. The amount is
part of the payment address itself, so a customer cannot pay less, and a page cannot change the
price after the code is scanned. Paying a different amount would send it to an address this code
is not watching. [What happens when you pay with a wallet](/learn/paying-with-a-wallet) walks
through it step by step.

## The seller knows who paid, and for what

A wallet shows incoming money as a list of amounts from strangers' addresses. Matching them to
sales is done by eye: which 4.50 was the coffee, and which was the croissant?

A Payvand code watches for its own payment. It turns **Paid** by itself the moment the money lands,
on the seller's screen and the customer's, and keeps the payer's address and the transaction as a
receipt. There is no "I sent it, look" screenshot to take on trust. A screenshot can be faked, but
a code that has turned Paid cannot.

## The customer does not need SOL

Sending USDC from a wallet needs a little SOL for the network fee. A customer with plenty of USDC
and no SOL cannot pay at all, which surprises most people the first time. With USDC or EURC on
Payvand, our relayer pays the network fee and is repaid out of the fee in the price, so the
customer only needs the coin they are paying in. [What a payment costs](/learn/what-a-payment-costs)
shows where every part of that goes.

## And it is still your wallet

None of this needs Payvand to hold anything. There is no Payvand balance and no seed phrase to
keep, and the app has no key that can move your funds. Every payment goes straight to the seller's
own wallet. Payvand is the layer on top of the wallet you already trust: it adds the checks and
the bookkeeping, and the money stays where it was.
