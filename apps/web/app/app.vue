<script setup>
useHead({
  meta: [
    { name: 'viewport', content: 'width=device-width, initial-scale=1' }
  ],
  link: [
    { rel: 'icon', type: 'image/png', href: '/logo.png' }
  ],
  htmlAttrs: {
    lang: 'en'
  }
})

const title = 'Payvand — Solana QR payments'
const description = 'Payvand makes taking payment simple: sellers create a QR code, customers scan it and pay from their wallet — no card reader, no account setup for the customer.'

useSeoMeta({
  title,
  description,
  ogTitle: title,
  ogDescription: description,
  ogImage: '/logo.png',
  twitterCard: 'summary'
})

const { cluster } = useRuntimeConfig().public
const { user, signout } = useAuth()

async function onSignOut() {
  await signout()
  await navigateTo('/')
}
</script>

<template>
  <UApp>
    <!--
      The theme hides the centre slot and shows the burger toggle below `lg`,
      so the three slot groups already divide into "desktop header" and
      "mobile menu" without extra breakpoints: #left renders in both, the
      default slot only on desktop, #right only on desktop, and #body only
      inside the menu.
    -->
    <UHeader>
      <!-- Logo, product name and cluster. -->
      <template #left>
        <div class="flex items-center gap-2">
          <NuxtLink
            to="/"
            aria-label="Payvand home"
            class="focus-visible:outline-3 outline-primary/25 flex items-center gap-2 rounded-md p-1 -ms-1"
          >
            <img
              src="/logo.png"
              alt=""
              width="356"
              height="368"
              class="h-7 w-auto shrink-0"
            >
            <span class="font-semibold">Payvand</span>
          </NuxtLink>

          <UBadge
            color="neutral"
            variant="subtle"
            size="sm"
          >
            {{ cluster }}
          </UBadge>
        </div>
      </template>

      <!-- Centred on desktop; `hidden lg:flex` comes from the theme. -->
      <template #default>
        <nav
          aria-label="Primary"
          class="flex items-center gap-1"
        >
          <UButton
            to="/sell"
            label="Sell"
            color="neutral"
            variant="ghost"
          />
          <UButton
            to="/pay"
            label="Pay"
            color="neutral"
            variant="ghost"
          />
        </nav>
      </template>

      <!-- Account controls on the right of the desktop header. -->
      <template #right>
        <nav
          aria-label="Account"
          class="hidden items-center gap-1 lg:flex"
        >
          <template v-if="user">
            <span
              class="text-muted inline-flex max-w-40 items-center gap-1.5 rounded-full bg-elevated/50 px-2.5 py-1 text-xs"
            >
              <UIcon
                name="i-lucide-user-round"
                class="size-3.5 shrink-0"
              />
              <span class="truncate">{{ user.email }}</span>
            </span>
            <UButton
              label="Sign out"
              color="neutral"
              variant="soft"
              icon="i-lucide-log-out"
              @click="onSignOut"
            />
          </template>
          <template v-else>
            <UButton
              to="/signin"
              label="Sign in"
              color="neutral"
              variant="ghost"
            />
            <UButton
              to="/signup"
              label="Sign up"
              color="primary"
              variant="soft"
              icon="i-lucide-user-plus"
            />
          </template>
        </nav>

        <UColorModeButton />
      </template>

      <!-- Mobile burger menu: every link that leaves the desktop header. -->
      <template #body>
        <nav
          aria-label="Menu"
          class="flex flex-col gap-1"
        >
          <UButton
            to="/sell"
            label="Sell"
            color="neutral"
            variant="soft"
            icon="i-lucide-qr-code"
            block
            class="justify-start"
          />
          <UButton
            to="/pay"
            label="Pay"
            color="neutral"
            variant="soft"
            icon="i-lucide-scan-line"
            block
            class="justify-start"
          />

          <USeparator class="my-3" />

          <template v-if="user">
            <span
              class="text-muted inline-flex max-w-full items-center gap-1.5 rounded-full bg-elevated/50 px-2.5 py-1 text-xs"
            >
              <UIcon
                name="i-lucide-user-round"
                class="size-3.5 shrink-0"
              />
              <span class="truncate">{{ user.email }}</span>
            </span>
            <UButton
              label="Sign out"
              color="neutral"
              variant="soft"
              icon="i-lucide-log-out"
              block
              class="justify-start"
              @click="onSignOut"
            />
          </template>
          <template v-else>
            <UButton
              to="/signin"
              label="Sign in"
              color="neutral"
              variant="soft"
              icon="i-lucide-log-in"
              block
              class="justify-start"
            />
            <UButton
              to="/signup"
              label="Sign up"
              color="primary"
              variant="soft"
              icon="i-lucide-user-plus"
              block
              class="justify-start"
            />
          </template>
        </nav>
      </template>
    </UHeader>

    <UMain>
      <NuxtPage />
    </UMain>

    <USeparator icon="i-lucide-scan-line" />

    <UFooter>
      <template #left>
        <p class="text-sm text-muted">
          Payvand • Simple payments on Solana • © {{ new Date().getFullYear() }}
        </p>
      </template>

      <template #right>
        <UButton
          to="https://solana.com/docs"
          target="_blank"
          icon="i-lucide-book-open"
          aria-label="Solana docs"
          color="neutral"
          variant="ghost"
        />
      </template>
    </UFooter>
  </UApp>
</template>
