import type { Ref } from 'vue'
import { isInstalled, isIosTab } from '~/utils/installPrompt'

/**
 * Push notifications for this browser.
 *
 * Permission can only be asked for from a user gesture, so nothing here touches
 * `Notification.requestPermission` until someone flips the switch on the
 * settings page. The subscription itself belongs to the browser — the service
 * worker receives the messages — and the server keeps the other half so a cron
 * can reach a phone whose app is closed.
 *
 * Subscribing always re-posts the subscription, even when the browser already
 * had one: a push service can rotate it, and the page coming back is the only
 * chance to tell the server the new endpoint.
 */
export function usePushNotifications() {
  const config = useRuntimeConfig()
  const base = config.public.apiBase
  const { token } = useAuth()
  const { t, locale } = useI18n()

  /** The browser has the three APIs this needs (iOS only after installing). */
  const supported = ref(false)
  const permission = ref<NotificationPermission>('default')
  const subscribed = ref(false)
  const busy = ref(false)
  const failure = ref('')
  /**
   * iPhone and iPad only offer Web Push to an app opened from the Home Screen,
   * so in a tab there is nothing to switch on until Payvand is installed. Kept
   * a ref set on mount, not a computed: the server cannot know the platform, and
   * an install prompt that appears before hydration would flicker.
   */
  const needsInstall = ref(false)

  function headers(): Record<string, string> {
    return authHeaders()
  }

  /** The VAPID key, or a clear failure when the server has none configured. */
  async function publicKey(): Promise<string> {
    const response = await $fetch<{ publicKey: string | null }>(`${base}/api/push/key`)
    if (!response.publicKey) throw new Error(t('settings.notificationsUnconfigured'))
    return response.publicKey
  }

  /** base64url as the browser's `applicationServerKey` wants it. */
  function decodeBase64Url(value: string): ArrayBuffer {
    const padded = value.replace(/-/g, '+').replace(/_/g, '/')
    const paddedWithEquals = padded.padEnd(padded.length + ((4 - (padded.length % 4)) % 4), '=')
    const binary = atob(paddedWithEquals)
    const bytes = new Uint8Array(binary.length)
    for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i)
    return bytes.buffer as ArrayBuffer
  }

  async function sendSubscription(subscription: PushSubscription): Promise<void> {
    const json = subscription.toJSON()
    await $fetch(`${base}/api/push/subscribe`, {
      method: 'POST',
      headers: headers(),
      body: {
        endpoint: subscription.endpoint,
        keys: { p256dh: json.keys?.p256dh, auth: json.keys?.auth },
        locale: locale.value
      }
    })
  }

  /**
   * An iPhone or iPad in a tab: iOS gives push only to the Home Screen app.
   * `navigator.standalone` and the display mode both answer "installed", and
   * iPadOS calls itself a Mac — `isIosTab` owns those details.
   */
  function needsIosInstall(): boolean {
    return isIosTab(
      navigator.userAgent,
      navigator.maxTouchPoints,
      isInstalled(
        (navigator as Navigator & { standalone?: boolean }).standalone,
        query => window.matchMedia(query).matches
      )
    )
  }

  /** Reads what the browser already has, and heals the server's copy of it. */
  async function detect(): Promise<void> {
    if (!import.meta.client) return
    needsInstall.value = needsIosInstall()
    supported.value
      = 'serviceWorker' in navigator && 'PushManager' in window && 'Notification' in window
    if (!supported.value) return

    permission.value = Notification.permission
    try {
      const registration = await navigator.serviceWorker.getRegistration()
      const subscription = await registration?.pushManager.getSubscription()
      subscribed.value = Boolean(subscription)
      if (subscription && token.value) await sendSubscription(subscription)
    } catch {
      // A browser that refuses to answer simply shows the switch as off.
    }
  }

  async function enable(): Promise<void> {
    failure.value = ''
    busy.value = true
    try {
      const granted = await Notification.requestPermission()
      permission.value = granted
      if (granted !== 'granted') {
        failure.value = t('settings.notificationsBlocked')
        return
      }

      const registration = await navigator.serviceWorker.ready
      const existing = await registration.pushManager.getSubscription()
      const subscription = existing
        ?? await registration.pushManager.subscribe({
          userVisibleOnly: true,
          applicationServerKey: decodeBase64Url(await publicKey())
        })
      await sendSubscription(subscription)
      subscribed.value = true
    } catch (error) {
      failure.value = error instanceof Error && error.message
        ? error.message
        : t('settings.notificationsFailed')
    } finally {
      busy.value = false
    }
  }

  async function disable(): Promise<void> {
    failure.value = ''
    busy.value = true
    try {
      const registration = await navigator.serviceWorker.ready
      const subscription = await registration.pushManager.getSubscription()
      if (subscription) {
        // The server first: a browser that drops the subscription before the
        // request lands would leave a row nobody can ever cancel.
        await $fetch(`${base}/api/push/unsubscribe`, {
          method: 'POST',
          headers: headers(),
          body: { endpoint: subscription.endpoint }
        })
        await subscription.unsubscribe()
      }
      subscribed.value = false
    } catch (error) {
      failure.value = error instanceof Error && error.message
        ? error.message
        : t('settings.notificationsFailed')
    } finally {
      busy.value = false
    }
  }

  /** Proves the whole path end to end with the notification the person is owed. */
  async function sendTest(): Promise<void> {
    failure.value = ''
    busy.value = true
    try {
      await $fetch(`${base}/api/push/test`, { method: 'POST', headers: headers() })
    } catch (error) {
      failure.value = error instanceof Error && error.message
        ? error.message
        : t('settings.notificationsFailed')
    } finally {
      busy.value = false
    }
  }

  onMounted(detect)

  /**
   * Off when the browser cannot, when permission was denied for good, or when an
   * iPhone or iPad has to open the app from its Home Screen icon first.
   */
  const available = computed(
    () => supported.value && permission.value !== 'denied' && !needsInstall.value
  )

  return {
    supported,
    available,
    needsInstall,
    permission,
    subscribed: subscribed as Readonly<Ref<boolean>>,
    busy,
    failure,
    enable,
    disable,
    sendTest
  }
}
