<script setup lang="ts">
/**
 * The shops subscription's payment sheet, in a dialog over whichever page
 * started the purchase.
 *
 * The sheet is the ordinary checkout — the same one a shared invoice link
 * opens — because it already carries every way to pay: an allowance with no
 * wallet approval, a connected wallet, or a wallet to pick. What changes is
 * where it lives. A subscription is a two-tap purchase from a card the seller
 * is looking at, so paying it should not hand them to a checkout page and
 * leave them there; the dialog closes onto the card, now showing the paid
 * period, and the invoice never becomes a page of its own.
 *
 * The invoice is in the purchase state rather than in this component: the
 * subscription card and the New shop dialog both start the same purchase, and
 * this is the one place either of them opens it.
 */
const purchase = useSubscriptionPurchase()

const open = computed({
  get: () => Boolean(purchase.checkout.value),
  set: (value) => {
    // Closing without paying charges nothing; the invoice is simply dropped.
    if (!value) purchase.close()
  }
})

// The purchase state outlives this page. Leaving with the sheet still open
// drops the invoice — nothing was paid, and no other page can finish it — so
// coming back does not reopen a sheet for a purchase the seller walked away
// from. A payment that did go through is in localStorage, and the next visit
// confirms it.
onBeforeUnmount(() => {
  purchase.close()
})
</script>

<template>
  <UModal
    v-model:open="open"
    :title="$t('subscription.title')"
    :description="$t('subscription.description')"
    :transition="false"
    :ui="{ content: 'max-w-md' }"
    unmount-on-hide
  >
    <template #body>
      <PaymentCheckout
        v-if="purchase.checkout.value"
        :source="purchase.checkout.value.source"
        embedded
        :permanent="false"
        :announce-result="false"
        @settled="purchase.complete()"
      />
    </template>
  </UModal>
</template>
