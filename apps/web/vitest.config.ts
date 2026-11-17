import vue from '@vitejs/plugin-vue'
import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vitest/config'

// `~` is Nuxt's alias for the app directory; the tests that reach into
// composables need it to resolve the same way the app does. Components are
// compiled here too, so `HoldToPayButton.test.ts` mounts the shipped button
// rather than a copy of it.
export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '~': fileURLToPath(new URL('./app', import.meta.url))
    }
  }
})
