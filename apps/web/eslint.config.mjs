// @ts-check
import withNuxt from './.nuxt/eslint.config.mjs'
import betterTailwindcss from 'eslint-plugin-better-tailwindcss'
import { getDefaultAttributes } from 'eslint-plugin-better-tailwindcss/api/defaults'

export default withNuxt(
  {
    ignores: [
      'app/assets/**/*.json'
    ]
  },
  betterTailwindcss.configs['correctness-error'],
  // The logo's SVG parts carry their own names as classes (`.wipe`, `.dot`,
  // `.piece`) and are styled in the component's scoped block, so they are not
  // Tailwind utilities and the plugin cannot resolve them.
  {
    files: ['app/components/PayvandLogoDraw.vue'],
    rules: { 'better-tailwindcss/no-unknown-classes': 'off' }
  },
  {
    settings: {
      'better-tailwindcss': {
        entryPoint: 'app/assets/css/main.css',
        attributes: [
          ...getDefaultAttributes(),
          ['^v-bind:ui$', [{ match: 'objectValues' }]]
        ]
      }
    }
  }
)
