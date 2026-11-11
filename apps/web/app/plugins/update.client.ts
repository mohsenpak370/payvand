/**
 * A new deployment leaves a service worker waiting. It used to take over at
 * once, which reloaded the page under the visitor — on the checkout that can
 * land in the middle of an approval. The worker waits now, and this offers the
 * update as a toast the person can accept when they are ready.
 *
 * Three details make this work from a plugin: `useI18n()` throws outside a
 * component, so the copy comes from `appT` when the toast is built; `$pwa` only
 * exists once the PWA module's own plugin has run, so the listener is wired on
 * `app:mounted`; and the duration is infinite, because a duration of zero
 * closes the toast before it is ever painted.
 */
export default defineNuxtPlugin(() => {
  const nuxtApp = useNuxtApp()

  nuxtApp.hook('app:mounted', () => {
    // `$pwa` is the module's reactive state; its refs are already unwrapped.
    const pwa = (nuxtApp as { $pwa?: { needRefresh: boolean, updateServiceWorker: (reload?: boolean) => Promise<void> } }).$pwa
    if (!pwa) return

    const toast = useToast()
    let offered = false

    watch(() => pwa.needRefresh, (waiting) => {
      if (!waiting || offered) return
      offered = true
      toast.add({
        title: appT('app.update.title'),
        description: appT('app.update.description'),
        duration: Number.POSITIVE_INFINITY,
        actions: [{
          label: appT('app.update.action'),
          onClick: () => {
            void pwa.updateServiceWorker(true)
          }
        }]
      })
    }, { immediate: true })
  })
})
