<script setup lang="ts">
useSeoMeta({
  title: 'Terms of use — Payvand',
  description: 'What Payvand does, what it does not do, and the handful of rules that come with using it: payments go wallet to wallet, we never hold the money, and a paid transaction cannot be taken back.'
})

const updated = '8 November 2026'
</script>

<template>
  <UContainer class="py-8 lg:py-12">
    <UPage>
      <UPageHeader
        title="Terms of use"
        description="What Payvand does, what it does not do, and the handful of rules that come with using it."
      />

      <UPageBody class="max-w-3xl">
        <div class="space-y-10">
          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              What Payvand is
            </h2>
            <p class="text-muted leading-relaxed">
              Payvand turns an amount into a payment code. A seller types in what something costs,
              the app builds a QR code, and a customer scans it with a Solana wallet and approves
              one transaction. The money moves from the customer's wallet to the seller's wallet
              over the Solana network — there is no account in the middle and no card processor.
            </p>
            <p class="text-muted leading-relaxed">
              Using the app means you accept these terms. If you do not, please do not use it.
            </p>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              We never hold the money
            </h2>
            <ul class="text-muted list-disc space-y-2 ps-5 leading-relaxed">
              <li>
                Payvand does not hold, receive or pass on your funds at any point, and has no way to
                move, freeze or take them.
              </li>
              <li>
                We cannot cancel, reverse or refund a payment. Only the recipient can send money
                back, from their own wallet.
              </li>
              <li>
                We never ask for a seed phrase or a private key, and nobody working on Payvand will
                ever ask you for one. Anyone who does is not us.
              </li>
              <li>
                The only thing the app can sign without your wallet is the practice budget, with a
                key your own browser creates and keeps (see the
                <NuxtLink
                  to="/privacy"
                  class="text-primary underline-offset-4 hover:underline"
                >privacy policy</NuxtLink>).
              </li>
            </ul>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              Your account
            </h2>
            <ul class="text-muted list-disc space-y-2 ps-5 leading-relaxed">
              <li>
                An account is an email address and a password. The password is stored as a one-way
                hash, never as the password itself.
              </li>
              <li>
                Keep the password to yourself. What happens under your account is your
                responsibility, so tell us if you think someone else has it.
              </li>
              <li>
                Link a wallet address you actually control: that link is how the app recognises your
                shops, your payments and your discount codes.
              </li>
              <li>
                You can close your account by asking us (see Contact). Removing an account also
                removes its shops, products, payment codes, discount codes and sessions.
              </li>
              <li>
                You must be old enough to agree to these terms where you live, and at least 13.
              </li>
            </ul>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              If you are selling
            </h2>
            <ul class="text-muted list-disc space-y-2 ps-5 leading-relaxed">
              <li>
                You are the seller of record. Prices, descriptions, taxes, receipts, refunds and
                consumer law are yours to get right, not ours.
              </li>
              <li>
                A payment that lands is final. If you want to give money back, send it back from
                your wallet — we cannot do it for you.
              </li>
              <li>
                What you publish is public. A payment code carries your wallet address, the amount,
                a reference, and the item name and memo you typed. Do not put anything private in
                those fields.
              </li>
              <li>
                Do not sell anything you are not allowed to sell where you are, and do not use
                Payvand for fraud, money laundering or to get around sanctions.
              </li>
            </ul>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              If you are paying
            </h2>
            <ul class="text-muted list-disc space-y-2 ps-5 leading-relaxed">
              <li>
                Read the amount and the address your wallet shows before you approve. The wallet is
                the last place anything can be checked, and it is telling you the truth.
              </li>
              <li>
                A confirmed payment cannot be undone by us, by the app, or by the seller's
                goodwill alone.
              </li>
              <li>
                A price shown in your own currency is an indicative conversion at a market rate, not
                a quote from us, and the network settles in SOL.
              </li>
              <li>
                Only pay codes from sellers you trust. A QR code is just data: anyone can print it,
                edit it or send you one.
              </li>
            </ul>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              Fees
            </h2>
            <p class="text-muted leading-relaxed">
              Payvand charges nothing for making or scanning a payment code. The Solana network
              charges a small fee for the transaction, and a payment creates a record on chain that
              costs a little rent — both are paid from the customer's wallet, and the wallet shows
              the total before you approve anything.
            </p>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              Test money
            </h2>
            <p class="text-muted leading-relaxed">
              When the badge in the corner says <strong class="text-default font-semibold">devnet</strong>,
              the app is running on Solana's test network and the SOL involved is not worth anything.
              It is there so you can practise. It is not a promise that real money behaves the same
              way, and nothing on devnet has value.
            </p>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              Fair use
            </h2>
            <p class="text-muted leading-relaxed">
              Do not attack the service or work around its limits: no scanning for weaknesses
              without asking first, no automated guessing of passwords or discount codes, no
              scraping, no hammering the API. We may suspend or remove an account that does any of
              that, and we may report it.
            </p>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              Availability, and what we do not promise
            </h2>
            <ul class="text-muted list-disc space-y-2 ps-5 leading-relaxed">
              <li>
                Payvand is provided as it is. We do not promise it will be available, that it will
                not lose data, or that it will keep working the same way — it may change or be
                discontinued.
              </li>
              <li>
                Nothing here is financial, investment, tax or legal advice.
              </li>
              <li>
                To the extent the law allows, we are not liable for indirect or consequential loss,
                for a payment you approved in your wallet, or for what a seller does or sells. Our
                total liability is limited to what you have paid us to use Payvand, which is
                nothing.
              </li>
              <li>
                Some places do not allow limits like these. Where that is the case, they apply only
                as far as the law there allows.
              </li>
            </ul>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              Changes, and contact
            </h2>
            <p class="text-muted leading-relaxed">
              We will update this page when the app changes, and change the date at the bottom.
              Using Payvand after a change means you accept the new version.
            </p>
            <p class="text-muted leading-relaxed">
              Questions about these terms: reply to any email Payvand has sent you. The address it
              came from reaches whoever runs this.
            </p>
          </section>

          <p class="text-dimmed border-default border-t pt-6 text-sm">
            Last updated {{ updated }}. This page is written to describe what the software actually
            does, in plain words.
          </p>
        </div>
      </UPageBody>
    </UPage>
  </UContainer>
</template>
