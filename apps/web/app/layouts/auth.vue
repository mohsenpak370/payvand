<script setup lang="ts">
const { t } = useI18n()
const { cluster } = useRuntimeConfig().public

// This layout has no app header, so the brand row carries the raw cluster
// name itself; the copy below says what that cluster means for whoever is
// about to sign in.
const mode = computed(() =>
  cluster === 'devnet' || cluster === 'testnet'
    ? t('common.testMode')
    : t('common.liveMode')
)

const points = computed(() => [
  {
    icon: 'i-lucide-qr-code',
    title: t('authLayout.points.code.title'),
    description: t('authLayout.points.code.description')
  },
  {
    icon: 'i-lucide-scan-line',
    title: t('authLayout.points.scan.title'),
    description: t('authLayout.points.scan.description')
  },
  {
    icon: 'i-lucide-zap',
    title: t('authLayout.points.paid.title'),
    description: t('authLayout.points.paid.description')
  }
])
</script>

<template>
  <UMain
    class="grid lg:min-h-[calc(100svh_-_var(--ui-header-height))] lg:grid-cols-2"
    :style="{ '--ui-header-height': '0px' }"
  >
    <!--
      UMain's own min-height is `calc(100vh - var(--ui-header-height))`; with
      the header height zeroed here it fills the viewport, so the split stays
      flush with the app shell instead of leaving a dead band above the
      footer. The form column still grows past it on short viewports.
    -->
    <!--
      Brand half of the split. Desktop only: below `lg` the form column
      carries its own compact brand row instead of squeezing two columns.
    -->
    <section class="relative hidden overflow-hidden bg-blue-950 text-white lg:block">
      <div
        aria-hidden="true"
        class="absolute -top-24 -left-24 size-72 rounded-full bg-blue-500/30 blur-3xl"
      />
      <div
        aria-hidden="true"
        class="absolute -right-20 -bottom-32 size-96 rounded-full bg-blue-400/20 blur-3xl"
      />

      <div class="relative flex h-full flex-col gap-14 p-10 xl:p-14">
        <div class="flex items-center gap-3">
          <!--
            Mark and name are one link home. No white tile behind the mark:
            the icon carries its own dark squircle, and at `size-[4.95rem]`
            it fills the height the old `p-[0.675rem]` tile wrapped around a
            `size-[3.6rem]` logo.
          -->
          <NuxtLink
            to="/"
            :aria-label="$t('app.brandHome')"
            class="focus-visible:outline-3 outline-blue-300/60 flex items-center gap-3 rounded-md"
          >
            <img
              src="/logo.png"
              alt=""
              width="356"
              height="368"
              class="size-[4.95rem]"
            >
            <!--
              The cluster pill hangs off the name's top-right corner instead
              of standing on the baseline beside it, so it reads as a tag on
              the word rather than a third item in the row. Same badge as the
              app header (`size="sm"`, neutral/subtle) — only the position is
              local, so padding, type and radius stay identical everywhere.
            -->
            <span class="relative text-[2.025rem] font-bold">
              Payvand
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
        </div>

        <div class="my-auto max-w-lg">
          <p class="text-xs font-semibold tracking-widest uppercase text-blue-300">
            {{ mode }}
          </p>
          <h1 class="mt-3 text-4xl leading-tight font-bold xl:text-5xl">
            {{ $t('authLayout.headline') }}
          </h1>
          <p class="mt-4 text-base leading-relaxed text-white/70">
            {{ $t('authLayout.subtitle') }}
          </p>

          <ul class="mt-10 flex flex-col gap-5">
            <li
              v-for="point in points"
              :key="point.title"
              class="flex gap-3"
            >
              <span class="mt-0.5 flex size-9 shrink-0 items-center justify-center rounded-lg bg-white/10 ring-1 ring-white/10">
                <UIcon
                  :name="point.icon"
                  class="size-4 text-blue-300"
                />
              </span>
              <span>
                <span class="block text-sm font-semibold">{{ point.title }}</span>
                <span class="mt-1 block text-sm text-white/60">{{ point.description }}</span>
              </span>
            </li>
          </ul>
        </div>
      </div>
    </section>

    <section class="flex items-center justify-center px-6 py-10 lg:px-10 xl:px-14">
      <div class="w-full max-w-sm">
        <div class="mb-8 flex items-center gap-2.5 lg:hidden">
          <NuxtLink
            to="/"
            :aria-label="$t('app.brandHome')"
            class="focus-visible:outline-3 outline-primary/25 flex items-center gap-2.5 rounded-md"
          >
            <img
              src="/logo.png"
              alt=""
              width="356"
              height="368"
              class="size-[4.95rem]"
            >
            <span class="relative text-[1.8rem] font-bold">
              Payvand
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
        </div>

        <slot />
      </div>
    </section>
  </UMain>
</template>
