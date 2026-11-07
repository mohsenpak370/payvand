---
title: Keep two wallets, one to spend and one to save
description: The wallet you connect to apps should hold pocket money, not savings. How to split them, and what a deliberately small balance protects you from.
category: safety
order: 4
---

A wallet is just a key, so there is nothing stopping you from having several. The useful split is
two: a **spending wallet** you connect to apps and pay from, and a **savings wallet** that no
website has ever seen. One is the till, the other is the safe.

Almost every large crypto loss starts with a wallet that was asked to do both jobs.

## Why one wallet for everything is risky

Every app you connect to gets a chance to ask your wallet for something — a payment, a signature,
sometimes an approval that lets it move a token later. Most of those requests are honest. The ones
that are not cost you whatever the wallet can reach.

That is the whole argument. If your daily wallet holds a week of spending, a bad site, a malicious
browser extension or a stolen laptop costs you that week. If it holds everything you own, the same
mistake costs you everything, and payments on this chain cannot be reversed.

## How to split them

1. **Savings first.** Move the amount you do not plan to spend into its own wallet, ideally on a
   hardware wallet, or at least with a phrase written on paper and never typed into a browser. This
   wallet gets connected to nothing. It does not browse, it does not "claim airdrops", it does not
   approve anything.
2. **Then a fresh spending wallet.** New seed phrase, kept separately from the savings phrase, used
   in the browser or on your phone for day-to-day payments. Fund it with roughly what you would
   carry in cash for a week.
3. **Move money deliberately.** Topping the spending wallet up from savings should be a decision you
   make, not something that happens automatically. Moving money back the other way should be rare;
   network fees make small round trips pointless.
4. **Name them.** Wallets that look identical are how people approve the wrong thing. Label the
   spending wallet "daily" and leave the savings one unnamed and unused.

If you only ever hold small amounts, one wallet is honestly fine. The split becomes worth the
friction once losing the balance would change your month.

## Where a small balance is a feature

A limit you can lose without stress is not a weaker setup — it is the safety mechanism. It caps the
damage of every mistake at once: the phishing site, the compromised extension, the phone left in a
taxi, the key on a laptop someone borrowed.

That is why an app allowance should be small on purpose. Payvand's monthly budget works the same
way: the amount you set is funded from your spending wallet into a program account derived from
your wallet, and a key held by your device can spend from it up to that limit and no further. If
that device is lost or compromised, the most that goes is the remainder of the month's limit — the
rest of the wallet is untouched, and closing the budget from your wallet takes back everything
left. [The allowance has its own guide](/learn/monthly-allowance-explained) if that is the part you
came here for.

## Habits worth keeping with a spending wallet

- **Never type a recovery phrase into a website.** One wallet per phrase, one phrase per backup. If
  the spending phrase sits next to the savings phrase, the split protects you less than you think.
- **Read what you approve.** A signature can be a payment or permission. The guide
  [Read before you sign](/learn/checking-before-you-sign) covers the four questions that catch
  almost all of it.
- **Keep the balance boring.** A spending wallet with a small balance is not worth attacking.
- **When something looks drained, check how it happened.** A site you approved, an extension you
  installed, or a key sitting on a device you no longer control. Fix that before refilling, or you
  are refilling someone else's till.
- **Separate devices help.** The savings wallet on a hardware device and the spending wallet in a
  browser extension on one machine means a compromised machine still cannot reach savings.

The point of all this is not paranoia. It is that you can use apps like this one with real money
while keeping the amount at risk small enough that a bad day is an inconvenience.
