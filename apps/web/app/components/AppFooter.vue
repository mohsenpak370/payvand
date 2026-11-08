<script setup lang="ts">
const { user } = useAuth()

/**
 * The brand block takes the grid's first column, so the groups sit beside it
 * rather than wrapping underneath. The account group changes with the session
 * — the same links the header shows, for a reader who scrolled instead of
 * looking up.
 */
const columns = computed(() => [
  {
    label: 'Product',
    children: [
      { label: 'Create a payment', to: '/sell' },
      { label: 'Scan and pay', to: '/scan' },
      { label: 'Learn about crypto', to: '/learn' }
    ]
  },
  ...(user.value
    ? []
    : [
        {
          label: 'Get started',
          children: [
            { label: 'Sign in', to: '/signin' },
            { label: 'Sign up', to: '/signup' }
          ]
        }
      ]),
  {
    label: 'Legal',
    children: [
      { label: 'Terms of use', to: '/terms' },
      { label: 'Privacy policy', to: '/privacy' }
    ]
  }
])
</script>

<template>
  <UFooter>
    <!--
      UFooter renders the top slot outside its container, so the columns bring
      their own to line up with the header and every page below.
    -->
    <template #top>
      <UContainer>
        <UFooterColumns :columns="columns">
          <template #left>
            <div class="max-w-sm">
              <NuxtLink
                to="/"
                aria-label="Payvand home"
                class="focus-visible:outline-3 outline-primary/25 inline-flex items-center gap-2 rounded-md"
              >
                <img
                  src="/logo.png"
                  alt=""
                  width="356"
                  height="368"
                  class="h-8 w-auto shrink-0"
                >
                <!--
                  The cluster pill repeats the header's, because whether the
                  money here is real decides how the rest of the page reads.
                -->
                <span class="relative text-xl font-bold">
                  <span class="text-primary">Pay</span>vand
                </span>
              </NuxtLink>

              <p class="text-muted mt-4 text-sm leading-relaxed">
                One QR code, scanned with a wallet. The money goes straight from the customer to
                the seller — never through us.
              </p>
            </div>
          </template>
        </UFooterColumns>
      </UContainer>
    </template>

    <template #left>
      <p class="text-muted text-sm">
        © {{ new Date().getFullYear() }} Payvand · Built on Solana · v0.1.0
      </p>
    </template>

    <template #right>
      <UButton
        to="https://solana.com/docs"
        target="_blank"
        icon="i-lucide-book-open"
        label="Solana docs"
        color="neutral"
        variant="ghost"
        size="sm"
      />
    </template>
  </UFooter>
</template>
