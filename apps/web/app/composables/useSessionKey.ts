import {
  createKeyPairFromPrivateKeyBytes,
  createSignerFromKeyPair,
  type Address,
  type KeyPairSigner
} from '@vue-solana/nuxt/kit'

/**
 * The session key is the one secret that can move money without a wallet
 * approval, so it is kept as a non-extractable WebCrypto `CryptoKey` in
 * IndexedDB rather than as bytes anywhere JavaScript can read. Its private
 * half cannot be exported again — not by this app, not by an injected script
 * — and browsers wrap such material with the OS keychain, so on macOS and
 * Windows a copied profile is not enough to use it elsewhere. The key is
 * still usable for signing while the page is compromised, but it cannot be
 * read out and reused.
 *
 * The only bytes that ever exist are the 32-byte seed, for the moment it takes
 * to import it, and that buffer is wiped immediately afterwards.
 */

/**
 * Earlier builds kept the raw seed as a JSON array here. It is adopted once,
 * imported as a non-extractable key, and deleted.
 */
const LEGACY_STORAGE_PREFIX = 'payvand.session.v1.'
const DATABASE_NAME = 'payvand.session'
const STORE_NAME = 'keys'
const DATABASE_VERSION = 1

/**
 * Some private-browsing modes refuse IndexedDB. The pair then lives in memory
 * for this page's lifetime instead of breaking the allowance flow — the key is
 * simply forgotten on reload, and the next payment asks for the wallet.
 */
const volatile = new Map<string, CryptoKeyPair>()

/**
 * Buyers whose key was dropped in this session, so a delete that has not
 * landed yet cannot be read back by the next `ensureSessionSigner`.
 */
const forgotten = new Set<string>()

let databasePromise: Promise<IDBDatabase> | null = null
let ed25519Support: boolean | null = null

/**
 * WebCrypto's Ed25519 support is what lets the key exist at all without raw
 * bytes: browsers without it (Chrome before 137, older embedded webviews)
 * cannot hold this key, so the flow says so instead of failing obscurely.
 */
async function canHoldSessionKey(): Promise<boolean> {
  if (ed25519Support !== null) return ed25519Support
  try {
    await crypto.subtle.generateKey({ name: 'Ed25519' }, false, ['sign', 'verify'])
    ed25519Support = true
  } catch {
    ed25519Support = false
  }
  return ed25519Support
}

function openDatabase(): Promise<IDBDatabase> {
  if (typeof indexedDB === 'undefined') {
    return Promise.reject(new Error(appT('errors.indexedDbUnavailable')))
  }
  databasePromise ??= new Promise<IDBDatabase>((resolve, reject) => {
    const request = indexedDB.open(DATABASE_NAME, DATABASE_VERSION)
    request.onupgradeneeded = () => {
      if (!request.result.objectStoreNames.contains(STORE_NAME)) {
        request.result.createObjectStore(STORE_NAME)
      }
    }
    request.onsuccess = () => resolve(request.result)
    request.onerror = () => reject(request.error ?? new Error(appT('errors.keyStoreOpen')))
  })
  return databasePromise
}

async function withStore<T>(
  mode: IDBTransactionMode,
  run: (store: IDBObjectStore) => IDBRequest<T>
): Promise<T> {
  const database = await openDatabase()
  return await new Promise<T>((resolve, reject) => {
    const transaction = database.transaction(STORE_NAME, mode)
    const request = run(transaction.objectStore(STORE_NAME))
    request.onsuccess = () => resolve(request.result)
    request.onerror = () => reject(request.error ?? new Error(appT('errors.keyStoreRequest')))
    transaction.onabort
      = () => reject(transaction.error ?? new Error(appT('errors.keyStoreAborted')))
  })
}

/** The stored key pair for this buyer, migrating a legacy plaintext one. */
async function readKeyPair(buyer: Address): Promise<CryptoKeyPair | null> {
  if (forgotten.has(buyer)) return null
  const inMemory = volatile.get(buyer)
  if (inMemory) return inMemory
  try {
    const stored = await withStore<CryptoKeyPair | undefined>('readonly', store => store.get(buyer))
    if (stored?.privateKey && stored.publicKey) return stored
  } catch {
    // No usable IndexedDB — fall back to a legacy key if this browser has one.
  }
  return await adoptLegacyKeyPair(buyer)
}

async function writeKeyPair(buyer: Address, keyPair: CryptoKeyPair): Promise<void> {
  forgotten.delete(buyer)
  volatile.set(buyer, keyPair)
  try {
    await withStore('readwrite', store => store.put(keyPair, buyer))
  } catch {
    // Memory-only: silent payments keep working until this page is closed.
  }
}

/**
 * Imports a seed an older build left in localStorage, stores the resulting
 * non-extractable pair, and removes the plaintext copy.
 */
async function adoptLegacyKeyPair(buyer: Address): Promise<CryptoKeyPair | null> {
  const storageKey = `${LEGACY_STORAGE_PREFIX}${buyer}`
  try {
    const raw = localStorage.getItem(storageKey)
    if (!raw) return null
    const seed = Uint8Array.from(JSON.parse(raw) as number[])
    if (seed.length !== 32) return null
    const keyPair = await createKeyPairFromPrivateKeyBytes(seed, false)
    seed.fill(0)
    await writeKeyPair(buyer, keyPair)
    localStorage.removeItem(storageKey)
    return keyPair
  } catch {
    return null
  }
}

/** The device's session key, or null when this device never opened one. */
export async function peekSessionSigner(buyer: Address): Promise<KeyPairSigner | null> {
  const keyPair = await readKeyPair(buyer)
  if (!keyPair) return null
  try {
    return await createSignerFromKeyPair(keyPair)
  } catch {
    return null
  }
}

/**
 * The session key for this buyer: loaded when this device has one, freshly
 * generated and persisted otherwise. Only the public address ever leaves the
 * device, and a copied browser profile yields no usable private key.
 */
export async function ensureSessionSigner(buyer: Address): Promise<KeyPairSigner> {
  const existing = await peekSessionSigner(buyer)
  if (existing) return existing

  if (!(await canHoldSessionKey())) {
    throw new Error(appT('errors.noSessionKeySupport'))
  }

  const seed = crypto.getRandomValues(new Uint8Array(32))
  const keyPair = await createKeyPairFromPrivateKeyBytes(seed, false)
  seed.fill(0)
  await writeKeyPair(buyer, keyPair)
  return await createSignerFromKeyPair(keyPair)
}

/** Drop the device's session key — used after the budget is closed. */
export function forgetSessionSigner(buyer: Address): void {
  forgotten.add(buyer)
  volatile.delete(buyer)
  try {
    localStorage.removeItem(`${LEGACY_STORAGE_PREFIX}${buyer}`)
  } catch {
    // Nothing to clean up.
  }
  void withStore('readwrite', store => store.delete(buyer)).catch(() => {
    // Memory-only key: already gone.
  })
}
