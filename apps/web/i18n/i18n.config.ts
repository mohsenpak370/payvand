export default defineI18nConfig(() => ({
  // A locale is allowed to be incomplete: anything it does not define is read
  // from English rather than shown as a key path. Locale codes themselves live
  // in nuxt.config.ts.
  fallbackLocale: 'en'
}))
