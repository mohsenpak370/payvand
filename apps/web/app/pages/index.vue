<script setup lang="ts">
const { cluster } = useRuntimeConfig().public

// "Running on devnet" means nothing to a shop owner; say what it costs them.
const ctaTitle = computed(() =>
  cluster === 'devnet' || cluster === 'testnet'
    ? 'Test mode — payments use free test SOL'
    : 'Live — accepting real SOL'
)
</script>

<template>
  <div>
    <UPageHero
      title="Simple payments on Solana"
      :ui="{ title: 'leading-tight' }"
      description="A seller creates a payment code in seconds, the customer scans it with their phone and approves the payment — that's it."
      :links="[{
        label: 'Create a payment QR',
        to: '/sell',
        icon: 'i-lucide-qr-code',
        size: 'xl'
      }, {
        label: 'Scan and pay',
        to: '/scan',
        icon: 'i-lucide-scan-line',
        size: 'xl',
        color: 'neutral',
        variant: 'subtle'
      }]"
    />

    <UPageSection
      id="how"
      title="How a payment moves"
      description="Three steps, no account setup for your customer."
      :features="[{
        icon: 'i-lucide-monitor-smartphone',
        title: 'Create a payment code',
        description: 'Enter the amount you want to charge and show the QR code. No card reader, no invoices, no monthly fees.'
      }, {
        icon: 'i-lucide-scan-qr-code',
        title: 'Your customer scans and pays',
        description: 'They scan the code, check the amount, and approve the payment from their own wallet. No app to install.'
      }, {
        icon: 'i-lucide-circle-check',
        title: 'Paid in seconds',
        description: 'The money arrives in your wallet and the code updates to say Paid — you both see it at the same time.'
      }]"
    />

    <UPageSection>
      <UPageCTA
        :title="ctaTitle"
        description="Open the seller page, enter an amount, and show the code to your customer. It takes about a minute."
        variant="subtle"
        :links="[{
          label: 'Start selling',
          to: '/sell',
          icon: 'i-lucide-arrow-right'
        }]"
      />
    </UPageSection>
  </div>
</template>
