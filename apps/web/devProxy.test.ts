// In dev, Nitro forwards only the /api prefixes listed in nuxt.config.ts's
// `devProxy` to the Axum server; anything else falls through to Nitro and
// comes back "Page not found". These checks keep that list in step with the
// API's route table and with what the app actually calls.
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'

const here = fileURLToPath(new URL('.', import.meta.url))

// Served by the API but never reached through the web origin.
const NOT_PROXIED = new Set(['/api/cron'])

function prefix(path: string): string {
  return path.split('/').slice(0, 3).join('/')
}

function proxied(): Set<string> {
  const config = readFileSync(join(here, 'nuxt.config.ts'), 'utf8')
  const block = config.slice(config.indexOf('devProxy:'))
  return new Set([...block.matchAll(/^\s+'(\/api\/[^']+)':\s*\{/gm)].map(m => m[1]!))
}

function apiPrefixes(): Set<string> {
  const router = readFileSync(join(here, '../api/src/router.rs'), 'utf8')
  const table = router.slice(router.indexOf('ROUTES'), router.indexOf('];'))
  return new Set([...table.matchAll(/"(\/api\/[^"]+)"/g)].map(m => prefix(m[1]!)))
}

// Nitro's own handlers under server/api, e.g. server/api/rates.get.ts → /api/rates.
function nitroPrefixes(): Set<string> {
  return new Set(
    readdirSync(join(here, 'server/api')).map(f => `/api/${f.split('.')[0]}`)
  )
}

function sourceFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    if (statSync(path).isDirectory()) return sourceFiles(path)
    return /\.(ts|vue)$/.test(name) && !name.includes('.test.') ? [path] : []
  })
}

function calledPrefixes(): Set<string> {
  const found = new Set<string>()
  for (const file of sourceFiles(join(here, 'app'))) {
    for (const m of readFileSync(file, 'utf8').matchAll(/\/api\/[a-zA-Z_.-]+/g)) {
      found.add(prefix(m[0]))
    }
  }
  return found
}

describe('dev proxy', () => {
  it('forwards every API route prefix', () => {
    const rules = proxied()
    const missing = [...apiPrefixes()].filter(p => !NOT_PROXIED.has(p) && !rules.has(p))
    expect(missing).toEqual([])
  })

  it('covers every /api path the app calls', () => {
    const rules = proxied()
    const nitro = nitroPrefixes()
    const missing = [...calledPrefixes()].filter(p => !rules.has(p) && !nitro.has(p))
    expect(missing).toEqual([])
  })

  it('does not proxy a path Nitro serves itself', () => {
    const rules = proxied()
    expect([...nitroPrefixes()].filter(p => rules.has(p))).toEqual([])
  })
})
