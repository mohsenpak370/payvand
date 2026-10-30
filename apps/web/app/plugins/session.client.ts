/**
 * Validates any session restored from localStorage after hydration, so the
 * header does not show stale account state for an expired token.
 */
export default defineNuxtPlugin((nuxtApp) => {
  const { ensureSession } = useAuth()

  nuxtApp.hook('app:mounted', () => {
    void ensureSession()
  })
})
