<script setup>
const { t } = useI18n()
const { cluster } = useRuntimeConfig().public
const { user, signout } = useAuth()
const { isAdmin } = useAdminAccess()
// Shops are a seller's own catalogue: shown to a signed-in account that has
// not switched them off in settings.
const { shopsEnabled } = useShopPreference()
const showShops = computed(() => !!user.value && shopsEnabled.value)

// The pages this layout wraps are prerendered in the default language, so a
// Persian reader loads English markup. Vue repairs the text while hydrating but
// leaves attributes alone, which is why the logo's "Payvand home" and the menu
// button's "Open menu" stayed English. Re-creating the chrome once the client
// knows which language the reader picked renders those attributes in it too.
const hydrated = ref(false)
onMounted(() => {
  hydrated.value = true
})

async function onSignOut() {
  await signout()
  await navigateTo('/')
}

const accountMenu = computed(() => [
  [
    { label: t('app.account.profile'), icon: 'i-lucide-circle-user-round', to: '/profile' },
    ...(showShops.value
      ? [{ label: t('app.account.shops'), icon: 'i-lucide-store', to: '/shops' }]
      : []),
    { label: t('app.account.settings'), icon: 'i-lucide-settings', to: '/settings' },
    ...(isAdmin.value
      ? [{ label: t('app.account.admin'), icon: 'i-lucide-shield-check', to: '/admin' }]
      : [])
  ],
  [
    { label: t('app.account.signOut'), icon: 'i-lucide-log-out', onSelect: onSignOut }
  ]
])
</script>

<template>
  <div>
    <UHeader :key="hydrated ? 'client' : 'prerendered'">
      <template #left>
        <NuxtLink
          to="/"
          :aria-label="$t('app.brandHome')"
          class="focus-visible:outline-3 outline-primary/25 flex items-center gap-2 rounded-md p-1 -ms-1"
        >
          <img
            src="/logo.png"
            alt=""
            width="356"
            height="368"
            class="h-10.5 w-auto shrink-0"
          >
          <span class="relative text-2xl font-bold">
            <span class="text-primary">{{ $t('app.brandWord.lead') }}</span>{{ $t('app.brandWord.rest') }}
            <UBadge
              color="success"
              variant="subtle"
              size="sm"
              class="absolute -top-2 inset-s-full ms-1"
            >
              {{ cluster }}
            </UBadge>
          </span>
        </NuxtLink>
      </template>

      <template #default>
        <nav
          :aria-label="$t('app.nav.primary')"
          class="flex items-center gap-6"
        >
          <NuxtLink
            to="/sell"
            class="text-muted hover:text-primary text-sm font-medium underline-offset-4 hover:underline"
          >
            {{ $t('app.nav.createPayment') }}
          </NuxtLink>
          <NuxtLink
            to="/scan"
            class="text-muted hover:text-primary text-sm font-medium underline-offset-4 hover:underline"
          >
            {{ $t('app.nav.scanAndPay') }}
          </NuxtLink>
          <NuxtLink
            to="/learn"
            class="text-muted hover:text-primary text-sm font-medium underline-offset-4 hover:underline"
          >
            {{ $t('app.nav.learn') }}
          </NuxtLink>
        </nav>
      </template>

      <template #right>
        <nav
          :aria-label="$t('app.nav.account')"
          class="hidden items-center gap-1 lg:flex"
        >
          <template v-if="user">
            <UDropdownMenu
              :items="accountMenu"
              :ui="{
                content: 'w-44'
              }"
            >
              <UButton
                color="neutral"
                variant="ghost"
                icon="i-lucide-circle-user-round"
                trailing-icon="i-lucide-chevrons-up-down"
                :label="user.email"
                class="max-w-44"
              />
            </UDropdownMenu>
          </template>
          <template v-else>
            <UButton
              to="/signin"
              :label="$t('app.auth.signIn')"
              color="neutral"
              variant="ghost"
            />
            <UButton
              to="/signup"
              :label="$t('app.auth.signUp')"
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
          :aria-label="$t('app.nav.menu')"
          class="flex flex-col gap-1"
        >
          <UButton
            to="/sell"
            :label="$t('app.nav.createPayment')"
            color="neutral"
            variant="soft"
            icon="i-lucide-qr-code"
            block
            class="justify-start"
          />
          <UButton
            to="/scan"
            :label="$t('app.nav.scanAndPay')"
            color="neutral"
            variant="soft"
            icon="i-lucide-scan-line"
            block
            class="justify-start"
          />
          <UButton
            to="/learn"
            :label="$t('app.nav.learnAboutCrypto')"
            color="neutral"
            variant="soft"
            icon="i-lucide-graduation-cap"
            block
            class="justify-start"
          />

          <USeparator class="my-3" />

          <template v-if="user">
            <span
              class="text-muted inline-flex max-w-full items-center gap-1.5 rounded-full bg-elevated/50 px-2.5 py-2 text-xs"
            >
              <UIcon
                name="i-lucide-user-round"
                class="size-3.5 shrink-0"
              />
              <span class="truncate">{{ user.email }}</span>
            </span>
            <UButton
              to="/profile"
              :label="$t('app.account.profile')"
              color="neutral"
              variant="soft"
              icon="i-lucide-circle-user-round"
              block
              class="justify-start"
            />
            <UButton
              v-if="showShops"
              to="/shops"
              :label="$t('app.account.shops')"
              color="neutral"
              variant="soft"
              icon="i-lucide-store"
              block
              class="justify-start"
            />
            <UButton
              to="/settings"
              :label="$t('app.account.settings')"
              color="neutral"
              variant="soft"
              icon="i-lucide-settings"
              block
              class="justify-start"
            />
            <UButton
              v-if="isAdmin"
              to="/admin"
              :label="$t('app.account.admin')"
              color="neutral"
              variant="soft"
              icon="i-lucide-shield-check"
              block
              class="justify-start"
            />
            <UButton
              :label="$t('app.account.signOut')"
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
              :label="$t('app.auth.signIn')"
              color="neutral"
              variant="soft"
              icon="i-lucide-log-in"
              block
              class="justify-start"
            />
            <UButton
              to="/signup"
              :label="$t('app.auth.signUp')"
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
      <slot />
    </UMain>

    <USeparator icon="i-lucide-scan-line" />
    <AppFooter :key="hydrated ? 'client' : 'prerendered'" />
  </div>
</template>
