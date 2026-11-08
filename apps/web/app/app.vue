<script setup>
useHead({
  meta: [
    { name: 'viewport', content: 'width=device-width, initial-scale=1' },
    // Colors the browser UI (and the task switcher on Android) to match the
    // app rather than the browser default.
    { name: 'theme-color', content: '#080E20' },
    // iOS ignores the manifest for the home-screen title and opens the app
    // full screen only when asked to; these are the two things it reads.
    { name: 'apple-mobile-web-app-title', content: 'Payvand' },
    { name: 'apple-mobile-web-app-capable', content: 'yes' }
  ],
  link: [
    { rel: 'icon', type: 'image/png', href: '/logo.png' },
    { rel: 'apple-touch-icon', sizes: '180x180', href: '/apple-touch-icon.png' }
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
const { isAdmin } = useAdminAccess()
// Shops are a seller's own catalogue: shown to a signed-in account that has
// not switched them off in settings.
const { shopsEnabled } = useShopPreference()
const showShops = computed(() => !!user.value && shopsEnabled.value)

// The auth pages carry their own brand row and want no chrome, so they opt
// out of the app header and footer with `header: false` / `footer: false`
// in their page meta.
const route = useRoute()
const showHeader = computed(() => route.meta.header !== false)
const showFooter = computed(() => route.meta.footer !== false)

async function onSignOut() {
  await signout()
  await navigateTo('/')
}

/**
 * The account menu: what used to be the Profile / email / Sign out cluster
 * in the header. `Admin` joins it only once the API has confirmed the
 * account is listed in `ADMIN_EMAILS`.
 */
const accountMenu = computed(() => [
  [
    { label: 'Profile', icon: 'i-lucide-circle-user-round', to: '/profile' },
    ...(showShops.value
      ? [{ label: 'Shops', icon: 'i-lucide-store', to: '/shops' }]
      : []),
    { label: 'Settings', icon: 'i-lucide-settings', to: '/settings' },
    ...(isAdmin.value
      ? [{ label: 'Admin', icon: 'i-lucide-shield-check', to: '/admin' }]
      : [])
  ],
  [
    { label: 'Sign out', icon: 'i-lucide-log-out', onSelect: onSignOut }
  ]
])
</script>

<template>
  <UApp>
    <!-- Links the web manifest that makes the app installable. -->
    <NuxtPwaManifest />

    <!--
      The theme hides the centre slot and shows the burger toggle below `lg`,
      so the three slot groups already divide into "desktop header" and
      "mobile menu" without extra breakpoints: #left renders in both, the
      default slot only on desktop, #right only on desktop, and #body only
      inside the menu.
    -->
    <UHeader v-if="showHeader">
      <!-- Logo, product name and cluster. -->
      <template #left>
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
            class="h-[2.625rem] w-auto shrink-0"
          >
          <!--
            The cluster pill hangs off the name's top-right corner instead of
            standing beside it, matching the brand row on the auth pages, so
            the cluster reads as a tag on the name rather than a third item.
          -->
          <span class="relative text-2xl font-bold">
            <span class="text-primary">Pay</span>vand
            <UBadge
              color="success"
              variant="subtle"
              size="sm"
              class="absolute -top-2 start-full ms-1"
            >
              {{ cluster }}
            </UBadge>
          </span>
        </NuxtLink>
      </template>

      <!--
        Centred on desktop; `hidden lg:flex` comes from the theme. Plain text
        links rather than buttons: these are navigation, and a hover underline
        reads as one without the button chrome competing with Sign up.
      -->
      <template #default>
        <nav
          aria-label="Primary"
          class="flex items-center gap-6"
        >
          <NuxtLink
            to="/sell"
            class="text-muted hover:text-primary text-sm font-medium underline-offset-4 hover:underline"
          >
            Create a payment
          </NuxtLink>
          <NuxtLink
            to="/scan"
            class="text-muted hover:text-primary text-sm font-medium underline-offset-4 hover:underline"
          >
            Scan and pay
          </NuxtLink>
          <NuxtLink
            to="/learn"
            class="text-muted hover:text-primary text-sm font-medium underline-offset-4 hover:underline"
          >
            Learn
          </NuxtLink>
        </nav>
      </template>

      <!-- Account controls on the right of the desktop header. -->
      <template #right>
        <nav
          aria-label="Account"
          class="hidden items-center gap-1 lg:flex"
        >
          <template v-if="user">
            <!-- Account: email on the trigger, everything else in the menu. -->
            <UDropdownMenu :items="accountMenu">
              <UButton
                color="neutral"
                variant="ghost"
                icon="i-lucide-circle-user-round"
                trailing-icon="i-lucide-chevrons-up-down"
                :label="user.email"
                class="max-w-56"
              />
            </UDropdownMenu>
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
            label="Create a payment"
            color="neutral"
            variant="soft"
            icon="i-lucide-qr-code"
            block
            class="justify-start"
          />
          <UButton
            to="/scan"
            label="Scan and pay"
            color="neutral"
            variant="soft"
            icon="i-lucide-scan-line"
            block
            class="justify-start"
          />
          <UButton
            to="/learn"
            label="Learn about crypto"
            color="neutral"
            variant="soft"
            icon="i-lucide-graduation-cap"
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
              to="/profile"
              label="Profile"
              color="neutral"
              variant="soft"
              icon="i-lucide-circle-user-round"
              block
              class="justify-start"
            />
            <UButton
              v-if="showShops"
              to="/shops"
              label="Shops"
              color="neutral"
              variant="soft"
              icon="i-lucide-store"
              block
              class="justify-start"
            />
            <UButton
              to="/settings"
              label="Settings"
              color="neutral"
              variant="soft"
              icon="i-lucide-settings"
              block
              class="justify-start"
            />
            <UButton
              v-if="isAdmin"
              to="/admin"
              label="Admin"
              color="neutral"
              variant="soft"
              icon="i-lucide-shield-check"
              block
              class="justify-start"
            />
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

    <!--
      The auth pages render neither header nor footer, so nothing occupies
      the header's 4rem. Zeroing the variable for them lets UMain and the
      auth layout fill the viewport instead of stopping 4rem short with an
      empty band where the footer used to be.
    -->
    <UMain :style="showHeader ? undefined : { '--ui-header-height': '0px' }">
      <!--
        NuxtPage does not render layouts on its own — without NuxtLayout
        around it, `layout: 'auth'` on the sign-in/sign-up pages would be
        ignored (and Nuxt warns NUXT_E4007). Routes without a matching
        layout render straight through.
      -->
      <NuxtLayout>
        <NuxtPage />
      </NuxtLayout>
    </UMain>

    <template v-if="showFooter">
      <USeparator icon="i-lucide-scan-line" />
      <AppFooter />
    </template>
  </UApp>
</template>
