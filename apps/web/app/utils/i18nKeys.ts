/**
 * Locale keys and the code that reads them, as data.
 *
 * A key nobody reads ships in five languages for nothing; a read whose key no
 * locale defines shows the raw dotted path to a buyer. Both are invisible until
 * someone greps, so this file turns the app's source into two lists the test
 * beside it can compare — the way `holdSlop` and `invoiceCost` keep a rule out
 * of a component.
 */

/** Every leaf key in a locale file, dotted: `checkout.settled.title`. */
export function flattenKeys(tree: Record<string, unknown>, prefix = ''): string[] {
  const keys: string[] = []
  for (const [name, value] of Object.entries(tree)) {
    const key = prefix ? `${prefix}.${name}` : name
    if (value && typeof value === 'object' && !Array.isArray(value)) {
      keys.push(...flattenKeys(value as Record<string, unknown>, key))
    } else {
      keys.push(key)
    }
  }
  return keys
}

export interface KeyReads {
  /** Keys named literally, in a `$t`/`t`/`appT` call or a `…Key` property. */
  keys: string[]
  /**
   * Prefixes of keys whose tail is chosen at runtime — `fiat.` for
   * `` $t(`fiat.${code}`) `` — so everything under them counts as read.
   */
  prefixes: string[]
}

/** A `$t`, `t` or `appT` call, however the app quotes its argument. */
const CALL = String.raw`(?:\$t|appT|\bt)\(\s*`
const PLAIN = new RegExp(String.raw`${CALL}(['"])([^'"\`]+)\1(?!\s*\+)`, 'g')
const TEMPLATE = new RegExp(String.raw`${CALL}\`([^\`]*)\``, 'g')
/**
 * A key held in a table rather than a call: `ariaKey: 'calculator.deleteLastDigit'`,
 * read later as `$t(k.ariaKey)`. The suffix is the convention; a property named
 * `…Key` holding a dotted string is a locale key.
 */
const HELD_KEY = /\b[a-z][a-zA-Z]*Key:\s*(['"])([^'"]+)\1/g

/**
 * What one source file reads. Call arguments that are built by concatenation —
 * `$t('fiat.' + code)` — are treated like a template, so the prefix covers the
 * subtree instead of the halves looking like two missing keys.
 */
export function readsIn(source: string): KeyReads {
  const keys: string[] = []
  const prefixes: string[] = []

  for (const match of source.matchAll(PLAIN)) keys.push(match[2]!)

  for (const match of source.matchAll(HELD_KEY)) keys.push(match[2]!)

  for (const match of source.matchAll(TEMPLATE)) {
    const body = match[1] ?? ''
    const cut = body.indexOf('${')
    if (cut === -1) keys.push(body)
    else prefixes.push(body.slice(0, cut))
  }

  // `$t('fiat.' + code)`: the literal half stops at the closing quote.
  for (const match of source.matchAll(new RegExp(String.raw`${CALL}['"]([^'"]+\.)['"]\s*\+`, 'g'))) {
    prefixes.push(match[1]!)
  }

  return { keys, prefixes }
}

export interface KeyAudit {
  /** Read by the app, defined by no locale. */
  undefined: string[]
  /** Defined in `en`, read by nothing. */
  unread: string[]
  /** The locale and the keys it has that `en` does not, or lacks. */
  drift: { locale: string, extra: string[], missing: string[] }[]
}

/**
 * Compares what the locales define against what the sources read. `reference` is
 * the locale every other one must match — `en`, because it is the one the rest
 * are translated from.
 */
export function auditKeys(input: {
  /** Locale name → its parsed file. */
  locales: Record<string, Record<string, unknown>>
  /** Every source read, concatenated per file. */
  reads: KeyReads[]
  reference: string
}): KeyAudit {
  const defined = new Map<string, string[]>()
  for (const [locale, tree] of Object.entries(input.locales)) {
    defined.set(locale, flattenKeys(tree))
  }

  const referenceKeys = defined.get(input.reference) ?? []
  const known = new Set(referenceKeys)

  const keys = new Set<string>()
  const prefixes: string[] = []
  for (const read of input.reads) {
    for (const key of read.keys) keys.add(key)
    prefixes.push(...read.prefixes)
  }

  const undefinedKeys = [...keys].filter(key => !known.has(key)).sort()
  const unread = referenceKeys
    .filter(key => !keys.has(key) && !prefixes.some(prefix => key.startsWith(prefix)))
    .sort()

  const drift = Object.keys(input.locales)
    .filter(locale => locale !== input.reference)
    .map((locale) => {
      const keys_ = new Set(defined.get(locale) ?? [])
      return {
        locale,
        extra: [...keys_].filter(key => !known.has(key)).sort(),
        missing: referenceKeys.filter(key => !keys_.has(key)).sort()
      }
    })
    .filter(entry => entry.extra.length || entry.missing.length)

  return { undefined: undefinedKeys, unread, drift }
}
