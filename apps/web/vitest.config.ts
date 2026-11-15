import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vitest/config'

// `~` is Nuxt's alias for the app directory; the tests that reach into
// composables need it to resolve the same way the app does.
export default defineConfig({
  resolve: {
    alias: {
      '~': fileURLToPath(new URL('./app', import.meta.url))
    }
  }
})
