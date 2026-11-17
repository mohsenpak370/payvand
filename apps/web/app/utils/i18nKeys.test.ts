import { readdirSync, readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'
import { auditKeys, flattenKeys, readsIn } from './i18nKeys'

/**
 * The five locale files and the app that reads them, kept honest.
 *
 * Deleting a screen leaves its strings behind, and adding a screen can reach for a
 * key nobody wrote; both are silent — one ships unused translations, the other
 * prints `checkout.settled.title` to a buyer. These cases fail on either, so the
 * next dead string is caught by the suite instead of by a grep.
 */

const LOCALES = ['en', 'de', 'es', 'fa', 'fr'] as const
const APP = new URL('..', import.meta.url)
const LOCALES_DIR = new URL('../../i18n/locales/', import.meta.url)

function readTree(file: URL): Record<string, unknown> {
  return JSON.parse(readFileSync(fileURLToPath(file), 'utf8')) as Record<string, unknown>
}

/** Every source file under `app/`, tests excluded — they read keys too, but of fixtures. */
function sources(): string[] {
  const found: string[] = []
  const walk = (dir: URL) => {
    for (const entry of readdirSync(fileURLToPath(dir), { withFileTypes: true })) {
      const child = new URL(`${entry.name}${entry.isDirectory() ? '/' : ''}`, dir)
      if (entry.isDirectory()) walk(child)
      else if (/\.(vue|ts)$/.test(entry.name) && !entry.name.endsWith('.test.ts')) {
        found.push(readFileSync(fileURLToPath(child), 'utf8'))
      }
    }
  }
  walk(APP)
  return found
}

describe('the locale files and their readers', () => {
  const locales = Object.fromEntries(LOCALES.map(code => [code, readTree(new URL(`${code}.json`, LOCALES_DIR))]))
  const audit = auditKeys({
    locales,
    reads: sources().map(readsIn),
    reference: 'en'
  })

  it('reads no key that no locale defines', () => {
    // A dotted path on the screen is a missing translation nobody noticed.
    expect(audit.undefined).toEqual([])
  })

  it('defines no key that nothing reads', () => {
    // Deleting a string is cheap; carrying it in five languages is not.
    expect(audit.unread).toEqual([])
  })

  it('keeps the four translations on the reference layout', () => {
    // A key added to `en` alone leaves the others printing the English fallback.
    expect(audit.drift).toEqual([])
  })
})

describe('flattenKeys', () => {
  it('names every leaf by its full path', () => {
    expect(flattenKeys({ checkout: { settled: { title: 'Paid', copy: 'Copy' } }, done: 'Done' }))
      .toEqual(['checkout.settled.title', 'checkout.settled.copy', 'done'])
  })

  it('keeps a plural string as one key', () => {
    expect(flattenKeys({ count: 'one | many' })).toEqual(['count'])
  })
})

describe('readsIn', () => {
  it('finds both quoting styles and every call form', () => {
    const source = [
      '<p>{{ $t("checkout.total") }}</p>',
      'const label = t(\'checkout.pay\')',
      'const error = appT("errors.notSent")',
      'n(\'notakey\') // an identifier ending in `n(`, not a read',
      'buildInstruction(\'settle_payment\') // an instruction name, not a key'
    ].join('\n')

    expect(readsIn(source).keys.sort())
      .toEqual(['checkout.pay', 'checkout.total', 'errors.notSent'])
  })

  it('counts a key held in a table, the way the calculator names its buttons', () => {
    const source = '{ ariaKey: \'calculator.deleteLastDigit\' }'

    expect(readsIn(source).keys).toEqual(['calculator.deleteLastDigit'])
  })

  it('treats a template key as a prefix, so its whole subtree counts as read', () => {
    const template = readsIn('$t(`fiat.${code}`)')
    const concatenated = readsIn('$t(\'budget.cost.\' + row.key)')

    expect(template.prefixes).toEqual(['fiat.'])
    expect(template.keys).toEqual([])
    expect(concatenated.prefixes).toEqual(['budget.cost.'])
  })
})

describe('auditKeys', () => {
  const locales = {
    en: { a: 'A', nested: { b: 'B' } },
    de: { a: 'A', nested: { b: 'B' } }
  }

  it('reports a read with no definition, and a definition with no read', () => {
    const audit = auditKeys({
      locales,
      reads: [{ keys: ['a', 'nested.c'], prefixes: [] }],
      reference: 'en'
    })

    expect(audit.undefined).toEqual(['nested.c'])
    expect(audit.unread).toEqual(['nested.b'])
  })

  it('counts a prefix as reading everything under it', () => {
    const audit = auditKeys({
      locales: { en: { fiat: { usd: 'USD', eur: 'EUR' } } },
      reads: [{ keys: [], prefixes: ['fiat.'] }],
      reference: 'en'
    })

    expect(audit.unread).toEqual([])
  })

  it('reports a locale that gained or lost a key', () => {
    const audit = auditKeys({
      locales: { en: { a: 'A', b: 'B' }, de: { a: 'A', c: 'C' } },
      reads: [{ keys: ['a', 'b'], prefixes: [] }],
      reference: 'en'
    })

    expect(audit.drift).toEqual([{ locale: 'de', extra: ['c'], missing: ['b'] }])
  })
})
