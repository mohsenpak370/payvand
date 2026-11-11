<script setup>
// The copy lives in i18n/locales/<code>.json, so the document language and
// direction come from the same place the words do: the browser and a screen
// reader pronounce the page as the language it is actually written in, and a
// right-to-left locale lays the whole app out that way.
const { t, locale, locales } = useI18n()
const activeLocale = computed(() => locales.value.find(entry => entry.code === locale.value))

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
  htmlAttrs: computed(() => ({
    lang: activeLocale.value?.language ?? String(locale.value),
    dir: activeLocale.value?.dir ?? 'ltr'
  }))
})

const title = computed(() => t('app.meta.title'))
const description = computed(() => t('app.meta.description'))

useSeoMeta({
  title,
  description,
  ogTitle: title,
  ogDescription: description,
  ogImage: '/logo.png',
  twitterCard: 'summary'
})
</script>

<template>
  <UApp>
    <!-- Links the web manifest that makes the app installable. -->
    <NuxtPwaManifest />

    <!--
      Each layout owns its own chrome: `default` renders the header and
      footer, `auth` the split sign-in screen, and `blank` the full-viewport
      payment and password-reset pages. Pages that want no app chrome pick
      the layout instead of toggling header/footer flags.
    -->
    <NuxtLayout>
      <NuxtPage />
    </NuxtLayout>
  </UApp>
</template>
