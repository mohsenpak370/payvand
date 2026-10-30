/**
 * `Buffer` is required by parts of the Solana stack when transactions are
 * built or serialized in the browser. Install it once, client side.
 */
import { installSolanaBufferPolyfill } from '@vue-solana/nuxt/buffer-polyfill'

export default defineNuxtPlugin(() => {
  installSolanaBufferPolyfill()
})
