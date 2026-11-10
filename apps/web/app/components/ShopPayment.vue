<script setup lang="ts">
import type { CartLine, ShopWithProducts } from '~/composables/useShop'
import { cartTotalLamports } from '~/composables/useShop'

// A payment made from the shop itself. The menu is the only thing on offer, so
// a code created here can charge for this shop's products and nothing else —
// no amount to type, no other shop's prices to get mixed in.
const props = defineProps<{ shopId: string }>()

const shopApi = useShopApi()
const api = usePaymentApi()
const { publicKey, connected } = useSolanaWallet()
const { cluster } = useRuntimeConfig().public

const shop = ref<ShopWithProducts | null>(null)
const loading = ref(true)
const cart = ref<CartLine[]>([])
const busy = ref(false)
const failure = ref('')
const qrPayload = ref('')
const copiedLink = ref(false)

const total = computed(() => cartTotalLamports(cart.value))
const canCreate = computed(() => connected.value && !busy.value && total.value > 0)
const shareLink = computed(() => (qrPayload.value ? paymentLinkPath(qrPayload.value) : ''))

async function load() {
  loading.value = true
  try {
    shop.value = await shopApi.get(props.shopId)
  } catch {
    // A shop that will not open has no menu to price; the rest of the page
    // still says what it could not load.
    shop.value = null
  } finally {
    loading.value = false
  }
}

async function create() {
  failure.value = ''
  const seller = publicKey.value
  if (!seller) {
    failure.value = 'Connect your seller wallet first.'
    return
  }
  if (total.value <= 0) {
    failure.value = 'Pick something from the shop first.'
    return
  }

  busy.value = true
  try {
    const created = await api.createPayment({
      seller,
      reference: createReference(),
      // Shop products are priced in lamports, so the code charges SOL.
      options: [{ currency: 'sol', amount: total.value }],
      memo: shop.value?.name ?? '',
      name: shop.value?.name ?? '',
      items: cart.value.map(line => ({
        name: line.product.name,
        quantity: line.quantity,
        amount_lamports: line.product.price_lamports
      })),
      cluster
    })
    qrPayload.value = created.qrPayload
    cart.value = []
  } catch (error) {
    failure.value = apiErrorMessage(error, 'Could not create the payment.')
  } finally {
    busy.value = false
  }
}

/** One more code from the same menu; the last one keeps its link. */
function reset() {
  qrPayload.value = ''
  failure.value = ''
  copiedLink.value = false
}

async function copyLink() {
  try {
    await navigator.clipboard.writeText(`${window.location.origin}${shareLink.value}`)
  } catch {
    return
  }
  copiedLink.value = true
  setTimeout(() => {
    copiedLink.value = false
  }, 2_000)
}

onMounted(load)
</script>

<template>
  <UCard>
    <template #header>
      <div class="flex flex-wrap items-center justify-between gap-2">
        <h2 class="text-base font-semibold">
          Create a payment
        </h2>
        <UButton
          v-if="qrPayload"
          label="New payment"
          color="neutral"
          variant="soft"
          size="xs"
          @click="reset"
        />
      </div>
    </template>

    <p
      v-if="loading"
      class="text-sm text-muted"
    >
      Loading this shop's menu…
    </p>

    <p
      v-else-if="!shop || !shop.products.length"
      class="text-sm text-muted"
    >
      Add a product above and this becomes a payable menu.
    </p>

    <div
      v-else-if="qrPayload"
      class="flex flex-col items-center gap-4"
    >
      <!-- 320 rather than 280: the styled code needs the extra pixels to stay
           readable once a phone's lens softens it (see PaymentQr). -->
      <PaymentQr
        :payload="qrPayload"
        :size="320"
      />

      <p class="text-center text-xs text-muted">
        Any phone camera opens this — and the page takes the payment.
      </p>

      <div class="flex flex-wrap items-center justify-center gap-2">
        <UButton
          :to="shareLink"
          target="_blank"
          rel="noopener"
          label="Open payment page"
          icon="i-lucide-external-link"
        />
        <UButton
          :icon="copiedLink ? 'i-lucide-check' : 'i-lucide-link'"
          :label="copiedLink ? 'Link copied' : 'Copy link'"
          color="neutral"
          variant="soft"
          @click="copyLink"
        />
      </div>
    </div>

    <div
      v-else
      class="flex flex-col gap-4"
    >
      <ProductPicker
        v-model="cart"
        :shops="[shop]"
      />

      <UAlert
        v-if="failure"
        color="error"
        variant="subtle"
        icon="i-lucide-circle-alert"
        title="Could not create the payment"
        :description="failure"
      />

      <UButton
        label="Create payment code"
        icon="i-lucide-qr-code"
        :loading="busy"
        :disabled="!canCreate"
        block
        @click="create"
      />

      <p
        v-if="!connected"
        class="text-center text-xs text-muted"
      >
        Connect your seller wallet to create the code.
      </p>
    </div>
  </UCard>
</template>
