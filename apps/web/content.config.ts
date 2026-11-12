import { defineCollection, defineContentConfig, z } from '@nuxt/content'

// One collection for the plain-language guides under /learn. `category` splits
// the index into its groups — the ideas, Solana itself, the app, and staying
// safe — and `order` is the reading order inside each group, so the index
// never depends on filenames.
//
// The guides sit under `content/<locale>/learn/` because the folder is the
// locale: the set of files is what a reader of that language gets. `prefix`
// hides the folder from the URL, so a guide is still /learn/<slug> in every
// language; the pages pick the collection that matches the current locale.
const learnSchema = z.object({
  title: z.string(),
  description: z.string(),
  category: z.enum(['basics', 'solana', 'payvand', 'safety', 'technical']),
  order: z.number(),
  // Technical guides assume some familiarity with keys and transactions,
  // and are labelled as such wherever they appear.
  level: z.enum(['beginner', 'technical']).optional()
})

export default defineContentConfig({
  collections: {
    learn: defineCollection({
      type: 'page',
      source: {
        include: 'en/learn/*.md',
        prefix: '/learn'
      },
      schema: learnSchema
    }),
    learnFa: defineCollection({
      type: 'page',
      source: {
        include: 'fa/learn/*.md',
        prefix: '/learn'
      },
      schema: learnSchema
    }),
    learnFr: defineCollection({
      type: 'page',
      source: {
        include: 'fr/learn/*.md',
        prefix: '/learn'
      },
      schema: learnSchema
    }),
    learnDe: defineCollection({
      type: 'page',
      source: {
        include: 'de/learn/*.md',
        prefix: '/learn'
      },
      schema: learnSchema
    }),
    learnEs: defineCollection({
      type: 'page',
      source: {
        include: 'es/learn/*.md',
        prefix: '/learn'
      },
      schema: learnSchema
    })
  }
})
