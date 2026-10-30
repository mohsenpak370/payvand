/**
 * Blocks /sell and /budget until the visitor has a valid account.
 * /pay is deliberately open — holding the payment link is enough to pay.
 */
export default defineNuxtRouteMiddleware(async (to) => {
  const { ensureSession } = useAuth()
  const signedIn = await ensureSession()
  if (signedIn) return

  return navigateTo({
    path: '/signin',
    query: { next: to.fullPath }
  })
})
