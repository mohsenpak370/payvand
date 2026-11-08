<script setup lang="ts">
useSeoMeta({
  title: 'Privacy policy — Payvand',
  description: 'What Payvand stores, why it stores it, and what stays in your browser: an email and a hashed password, the shops and codes you create, and no tracking, no ads and no wallet keys.'
})

const updated = '8 November 2026'
</script>

<template>
  <UContainer class="py-8 lg:py-12">
    <UPage>
      <UPageHeader
        title="Privacy policy"
        description="What Payvand stores, why it stores it, and what never leaves your browser."
      />

      <UPageBody class="max-w-3xl">
        <div class="space-y-10">
          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              The short version
            </h2>
            <ul class="text-muted list-disc space-y-2 ps-5 leading-relaxed">
              <li>
                An account is an email address and a password. We keep the email and a one-way hash
                of the password — never the password itself.
              </li>
              <li>
                We keep what you create: shops, products, payment codes and discount codes.
              </li>
              <li>
                No advertising, no analytics, no tracking pixels, and no selling or sharing your
                data for marketing. There is none of it in the app to switch off.
              </li>
              <li>
                Your wallet's keys never reach us, so we cannot move your money or read your
                balance without asking the network, as anyone can.
              </li>
              <li>
                Payments are recorded on a public blockchain: visible to anyone, permanently, and
                not deletable by us or by you.
              </li>
            </ul>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              What we store, and why
            </h2>
            <ul class="text-muted list-disc space-y-2 ps-5 leading-relaxed">
              <li>
                <strong class="text-default font-semibold">Your account.</strong> The email address
                (to sign in and to reach you), a hash of your password made with Argon2, and the
                wallet address once you link one (so the app knows which shops, payments and
                discount codes are yours).
              </li>
              <li>
                <strong class="text-default font-semibold">Sessions.</strong> A random token, kept
                hashed, with the time it was created and the time it expires. A session lasts 30
                days; signing out deletes it.
              </li>
              <li>
                <strong class="text-default font-semibold">What you create.</strong> Shop names and
                descriptions, product names, details and prices, saved payment codes, and discount
                codes with their percentages.
              </li>
              <li>
                <strong class="text-default font-semibold">Password resets.</strong> A hashed token
                that works once and expires after an hour.
              </li>
              <li>
                <strong class="text-default font-semibold">The waiting list.</strong> The email
                address you submit, until it is used or removed.
              </li>
              <li>
                <strong class="text-default font-semibold">Server logs.</strong> Errors are logged
                so the service can be fixed. The log does not contain passwords, and error messages
                shown to the app no longer contain database detail.
              </li>
            </ul>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              What other people can see
            </h2>
            <ul class="text-muted list-disc space-y-2 ps-5 leading-relaxed">
              <li>
                <strong class="text-default font-semibold">A payment code.</strong> It travels in a
                URL or a QR code and contains the seller's wallet address, the amount, a reference,
                the item name and the memo the seller typed. When the code was made while signed
                in, it also carries the seller's account id as a hint, so the checkout knows to offer
                that seller's discount codes. Anyone holding the code sees all of that.
              </li>
              <li>
                <strong class="text-default font-semibold">A shop.</strong> Shops have a public
                link: the name, description, products and prices on it are readable by anyone with
                the link.
              </li>
              <li>
                <strong class="text-default font-semibold">The blockchain.</strong> A payment writes
                the two wallet addresses, the amount and the time into a public ledger that anyone
                can read and nobody can edit.
              </li>
              <li>
                Your email address and password hash are not published anywhere and do not appear in
                a payment code.
              </li>
            </ul>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              Wallet keys are not ours to hold
            </h2>
            <ul class="text-muted list-disc space-y-2 ps-5 leading-relaxed">
              <li>
                Every real payment is approved inside your own wallet. Payvand never has the keys to
                it and cannot sign for it.
              </li>
              <li>
                The practice budget is signed with a key your browser generates and keeps as a
                non-extractable browser key: it can sign while you use the app, but it cannot be
                read back out — not by this app, not by a script on the page. It never leaves your
                device, and we have no copy.
              </li>
              <li>
                We never ask for a seed phrase and never store private keys. There is nothing here
                for a thief to take but an email address and a hash.
              </li>
            </ul>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              What is kept in your browser
            </h2>
            <ul class="text-muted list-disc space-y-2 ps-5 leading-relaxed">
              <li>
                <strong class="text-default font-semibold">Local storage.</strong> Your session
                token and a copy of your profile, so the header does not flicker while it loads; the
                currency you chose; the wallet you connected last; and your light or dark theme.
              </li>
              <li>
                <strong class="text-default font-semibold">IndexedDB.</strong> The practice budget's
                key, in the form described above.
              </li>
              <li>
                <strong class="text-default font-semibold">The installed app's cache.</strong> It
                keeps the app's own files so it opens offline. It deliberately never caches payment
                pages or payment status, so you are never shown a stale amount.
              </li>
              <li>
                There are no advertising or analytics cookies. Clearing this site's data signs you
                out on that device and forgets those choices.
              </li>
            </ul>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              Who else is involved
            </h2>
            <ul class="text-muted list-disc space-y-2 ps-5 leading-relaxed">
              <li>
                A hosting provider runs the website and the API.
              </li>
              <li>
                A hosted SQL database (libSQL) stores the accounts, shops and codes.
              </li>
              <li>
                The Solana network, reached through an RPC provider, is read for balances and payment
                status. Payments themselves are sent by your wallet, not by us.
              </li>
              <li>
                An email provider delivers the few messages the app sends: sign-in notices, password
                resets, and a note if someone tries to sign up with your address.
              </li>
            </ul>
            <p class="text-muted leading-relaxed">
              These providers handle the data only to run the service for us. We do not sell it, we
              do not share it for advertising, and there is no analytics network watching these
              pages.
            </p>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              How long we keep it
            </h2>
            <ul class="text-muted list-disc space-y-2 ps-5 leading-relaxed">
              <li>Your account and everything you create: until the account is deleted.</li>
              <li>Sessions: 30 days, then expired and swept away.</li>
              <li>Password reset links: one hour.</li>
              <li>Waiting list entries: until the invitation is used or the address is removed.</li>
              <li>
                Server logs: as long as the hosting provider keeps them, usually days to weeks.
              </li>
              <li>
                Anything written to the Solana blockchain: forever. Nobody can delete it, including
                us.
              </li>
            </ul>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              Deleting your data
            </h2>
            <p class="text-muted leading-relaxed">
              Ask us and we will remove the account: its email, password hash, linked wallet, shops,
              products, saved payment codes, discount codes, sessions and any pending reset link.
              Deleting the account does not, and cannot, remove payments already recorded on the
              blockchain — that history belongs to the network, not to us. There is no self-service
              delete button yet, so a person does it.
            </p>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              Children
            </h2>
            <p class="text-muted leading-relaxed">
              Payvand is not for anyone under 13, or under the age at which you can agree to the
              terms where you live. We do not knowingly keep an account for a child; if you think one
              exists, tell us and we will remove it.
            </p>
          </section>

          <section class="space-y-3">
            <h2 class="text-highlighted text-lg font-semibold">
              Changes, and contact
            </h2>
            <p class="text-muted leading-relaxed">
              If what the app stores changes, this page changes with it and the date at the bottom
              moves.
            </p>
            <p class="text-muted leading-relaxed">
              Questions about your data, or a deletion request: reply to any email Payvand has sent
              you. The address it came from reaches whoever runs this.
            </p>
          </section>

          <p class="text-dimmed border-default border-t pt-6 text-sm">
            Last updated {{ updated }}. This page is written to describe what the software actually
            does, in plain words. See also the
            <NuxtLink
              to="/terms"
              class="text-primary underline-offset-4 hover:underline"
            >terms of use</NuxtLink>.
          </p>
        </div>
      </UPageBody>
    </UPage>
  </UContainer>
</template>
