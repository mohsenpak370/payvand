/*
 * Payvand's service worker.
 *
 * Two jobs. It caches the build's hashed assets so the installed app opens
 * without the network, and it is the only place a push notification can arrive
 * and be shown: a page is not running when the phone is in someone's pocket.
 *
 * It deliberately caches no navigation responses. An invoice or a payment
 * status read from a cache is a stale answer to a question about money, so
 * every page load goes to the network even when the app is offline.
 */

// The build replaces this with the list of hashed files it produced.
const MANIFEST = self.__WB_MANIFEST || []

/**
 * One cache per build. The name carries a fingerprint of the file list, so a
 * new deployment installs beside the old worker's cache and activation deletes
 * everything else — no hashed file is ever left behind.
 */
function fingerprint(entries) {
  let hash = 0
  for (const entry of entries) {
    for (let i = 0; i < entry.url.length; i++) {
      hash = (hash * 31 + entry.url.charCodeAt(i)) | 0
    }
  }
  return Math.abs(hash).toString(36)
}

const CACHE = `payvand-precache-${fingerprint(MANIFEST)}`
const PRECACHED = new Set(MANIFEST.map(entry => entry.url))

self.addEventListener('install', (event) => {
  // No `skipWaiting()`: the update prompt in the app decides when the new
  // worker takes over, because taking over mid-checkout would reload the page
  // under someone who is approving a payment.
  event.waitUntil(
    caches.open(CACHE).then(cache => cache.addAll([...PRECACHED]))
  )
})

self.addEventListener('activate', (event) => {
  event.waitUntil((async () => {
    const names = await caches.keys()
    await Promise.all(
      names
        .filter(name => name.startsWith('payvand-precache-') && name !== CACHE)
        .map(name => caches.delete(name))
    )
    await self.clients.claim()
  })())
})

// `workbox-window` asks the waiting worker to take over when the person accepts
// the update toast.
self.addEventListener('message', (event) => {
  if (event.data && event.data.type === 'SKIP_WAITING') {
    self.skipWaiting()
  }
})

self.addEventListener('fetch', (event) => {
  const { request } = event
  if (request.method !== 'GET') return

  const url = new URL(request.url)
  if (url.origin !== self.location.origin) return
  // Pages are always live: see the note at the top.
  if (request.mode === 'navigate') return
  if (!PRECACHED.has(url.pathname)) return

  event.respondWith((async () => {
    const cache = await caches.open(CACHE)
    const hit = await cache.match(request, { ignoreSearch: true })
    return hit || fetch(request)
  })())
})

/** The payload the API sends: `{ title, body, url }`, already in the person's language. */
function readPush(event) {
  if (!event.data) return { title: 'Payvand', body: '', url: '/' }
  try {
    const parsed = event.data.json()
    return {
      title: typeof parsed.title === 'string' && parsed.title ? parsed.title : 'Payvand',
      body: typeof parsed.body === 'string' ? parsed.body : '',
      // Only a same-origin path: a notification is not a place to open
      // whatever URL a payload names.
      url: typeof parsed.url === 'string' && parsed.url.startsWith('/') && !parsed.url.startsWith('//')
        ? parsed.url
        : '/'
    }
  } catch {
    return { title: 'Payvand', body: event.data.text(), url: '/' }
  }
}

self.addEventListener('push', (event) => {
  const { title, body, url } = readPush(event)
  event.waitUntil(
    self.registration.showNotification(title, {
      body,
      // The large icon: the marker tile, drawn in colour on every platform
      // that has room for it.
      icon: '/pwa-192x192.png',
      // The status-bar badge is not a thumbnail of `icon`: Android keeps only
      // the alpha channel and paints it as a silhouette, so a full-colour
      // square came out as a filled square in the shade. This one is the mark
      // as white on transparent, 96px for a 4x screen. iOS ignores `badge` and
      // uses the installed app's own icon, which the manifest already sets.
      badge: '/badge-96x96.png',
      // One tag per topic, so a second reminder replaces the first instead of
      // stacking up in the notification shade.
      tag: 'payvand-allowance',
      renotify: false,
      data: { url }
    })
  )
})

self.addEventListener('notificationclick', (event) => {
  event.notification.close()
  const url = (event.notification.data && event.notification.data.url) || '/'

  event.waitUntil((async () => {
    const windows = await self.clients.matchAll({ type: 'window', includeUncontrolled: true })
    // An installed app is a single window: focus it and move it to the page the
    // notification is about, rather than opening a second copy.
    for (const client of windows) {
      if ('focus' in client) {
        await client.focus()
        if ('navigate' in client) {
          try {
            await client.navigate(url)
          } catch {
            // A window that cannot be navigated still got the person's attention.
          }
        }
        return
      }
    }
    await self.clients.openWindow(url)
  })())
})
