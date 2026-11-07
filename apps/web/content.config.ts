import { defineCollection, defineContentConfig, z } from '@nuxt/content'

// One collection for the plain-language guides under /learn. `category` splits
// the index into its groups — the ideas, Solana itself, the app, and staying
// safe — and `order` is the reading order inside each group, so the index
// never depends on filenames.
export default defineContentConfig({
  collections: {
    learn: defineCollection({
      type: 'page',
      source: 'learn/*.md',
      schema: z.object({
        title: z.string(),
        description: z.string(),
        category: z.enum(['basics', 'solana', 'payvand', 'safety', 'technical']),
        order: z.number(),
        // Technical guides assume some familiarity with keys and transactions,
        // and are labelled as such wherever they appear.
        level: z.enum(['beginner', 'technical']).optional()
      })
    })
  }
})
