/**
 * A new deployment leaves a service worker waiting. It used to take over at
 * once, which reloaded the page under the visitor — on the checkout that can
 * land in the middle of an approval. The worker waits now, and this offers the
 * update as a toast the person can accept when they are ready.
 *
 * The toast has no timeout: nothing is lost by leaving it there, and it
 * disappears on reload anyway.
 */
export default defineNuxtPlugin(() => {
  // `$pwa` is the module's reactive state; its refs are already unwrapped.
  const { $pwa } = useNuxtApp() as { $pwa?: { needRefresh: boolean, updateServiceWorker: (reload?: boolean) => Promise<void> } }
  if (!$pwa) return

  const { t } = useI18n()
  const toast = useToast()
  let offered = false

  watch(() => $pwa.needRefresh, (waiting) => {
    if (!waiting || offered) return
    offered = true
    toast.add({
      title: t('app.update.title'),
      description: t('app.update.description'),
      duration: 0,
      actions: [{
        label: t('app.update.action'),
        onClick: () => {
          void $pwa.updateServiceWorker(true)
        }
      }]
    })
  }, { immediate: true })
})
