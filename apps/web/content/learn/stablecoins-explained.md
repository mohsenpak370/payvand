---
title: Stablecoins, in plain words
description: Why a shop can quote prices in USDC or EURC without worrying about the price of the token moving, and what actually backs them.
category: basics
order: 3
---

Most crypto you hear about has a price that moves all day. That is a terrible property for a
coffee: if the shop quotes you one SOL and the price of SOL falls 5% before you pay, the shop
just lost 5%.

A **stablecoin** is a token designed to always be worth one unit of an ordinary currency. The two
you will see on Payvand are **USDC** (one dollar each) and **EURC** (one euro each), both issued
by Circle.

## What "backed" means

For every USDC in circulation, Circle holds about a dollar of cash and short-term government
debt in reserve, and publishes regular attestations of those reserves. EURC works the same way
against the euro. You can hand USDC back to Circle and receive dollars, which is what keeps the
price pinned to a dollar: if it traded at 97 cents, someone would buy it and redeem at par.

That is also the honest limit of the promise. A stablecoin is only as solid as its issuer and its
reserves. It is a claim on a company, not central-bank money, so "stable" means "pegged", not
"risk-free".

## How they get onto Solana

USDC and EURC on Solana are **SPL tokens**: standard tokens issued by a specific program on the
network. Each token has a mint address that identifies it, and each holder has a token account
holding a balance for that mint. Your wallet creates that account for you the first time you
receive the token, which costs a tiny amount of SOL in rent.

This matters for one practical reason: **there are fake tokens with familiar names**. A token
called "USDC" is not USDC. Wallets and apps avoid this by checking the mint address, not the
name. Payvand pins the two mints it accepts, so an invoice can only ever be settled with the real
USDC or EURC.

## Why shops like them

- **The price does not drift.** A 4.50 EURC invoice is worth 4.50 euros when it is issued and when
  it is paid, so there is no need to re-quote mid-conversation.
- **They settle like any other token.** Seconds, for a fraction of a cent, with no card fees and
  no chargebacks.
- **They are boring, which is the point.** A shop does not want to speculate on its own till.

## Why buyers like them

If you already hold euros or dollars in a stablecoin, paying with it does not touch your exposure
to the price of SOL or any other token. It behaves like a foreign-currency wallet: the balance is
what it says it is.

## Test tokens are not money

On a test network (Payvand's header shows **devnet**), USDC and EURC are free test tokens from a
faucet. They have the same name and the same mint behaviour, and they are worth exactly nothing.
That is deliberate: it is how you learn the motions — funding an allowance, paying an invoice,
withdrawing the rest — without risking a cent.
