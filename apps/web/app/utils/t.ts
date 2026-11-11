/**
 * A translation from outside a component's setup.
 *
 * `useI18n()` only works while a component or composable is being set up, but
 * some of the app's words are produced later: `statusLabel` renders a badge,
 * a clipboard callback explains itself, a signer throws. Those call the app's
 * one i18n instance instead, which is the same set of messages.
 */
type Translator = {
  t: (key: string, named?: Record<string, unknown>) => string
}

export function appT(key: string, named?: Record<string, unknown>): string {
  const i18n = useNuxtApp().$i18n as unknown as Translator
  return i18n.t(key, named ?? {})
}
