<script setup lang="ts">
// One shop, opened from /shops. Everything on this page belongs to the
// signed-in account: the API scopes each read and write to it.
//
// The page has two jobs, and they are used at different moments: taking money
// (a payment priced from the menu, and the codes already handed out) and
// keeping the shop itself (its name and its products). Stacked, the catalogue
// pushed the payment card out of sight and the payment card buried the
// catalogue; a tab each keeps whichever one is wanted in view.
const { t } = useI18n()

useSeoMeta({ title: t('shops.shopMetaTitle') })

definePageMeta({ middleware: 'auth' })

const route = useRoute()
const shopId = computed(() => String(route.params.id ?? ''))

const tabs = computed(() => [
  { value: 'payments' as const, label: t('shops.tabPayments'), icon: 'i-lucide-receipt' },
  { value: 'shop' as const, label: t('shops.tabShop'), icon: 'i-lucide-store' }
])

/** Money first: it is what the shop exists for. */
const tab = ref<'payments' | 'shop'>('payments')
</script>

<template>
  <UPageSection
    :title="$t('shops.shopTitle')"
    :description="$t('shops.shopDescription')"
  >
    <div class="mx-auto flex w-full max-w-3xl min-w-0 flex-col gap-4">
      <NuxtLink
        to="/shops"
        class="flex items-center gap-1.5 text-sm font-medium text-muted underline-offset-4 hover:text-default hover:underline"
      >
        <UIcon
          name="i-lucide-arrow-left"
          class="size-4"
        />
        {{ $t('shops.backToShops') }}
      </NuxtLink>

      <div
        role="tablist"
        class="flex gap-1 rounded-xl bg-elevated/50 p-1"
      >
        <button
          v-for="item in tabs"
          :id="`shop-tab-${item.value}`"
          :key="item.value"
          type="button"
          role="tab"
          :aria-selected="tab === item.value"
          :aria-controls="`shop-panel-${item.value}`"
          class="flex flex-1 items-center justify-center gap-2 rounded-lg px-3 py-2 text-sm font-medium transition-colors"
          :class="tab === item.value
            ? 'bg-default text-highlighted shadow-sm'
            : 'text-muted hover:text-default'"
          @click="tab = item.value"
        >
          <UIcon
            :name="item.icon"
            class="size-4"
            aria-hidden="true"
          />
          {{ item.label }}
        </button>
      </div>

      <!--
        Both panels stay mounted: a product halfway typed in the shop tab is
        still there after a look at the codes, and switching tabs costs no read.
      -->
      <div
        v-show="tab === 'payments'"
        id="shop-panel-payments"
        role="tabpanel"
        aria-labelledby="shop-tab-payments"
        class="flex flex-col gap-4"
      >
        <!-- The full payment form, priced from this shop's menu, on its own
             page so the code gets the room. -->
        <UCard>
          <div class="flex flex-wrap items-center justify-between gap-3">
            <div>
              <h2 class="text-base font-semibold">
                {{ $t('shops.createPaymentTitle') }}
              </h2>
              <p class="text-sm text-muted">
                {{ $t('shops.createPaymentDescription') }}
              </p>
            </div>
            <UButton
              :to="`/shops/${shopId}/pay`"
              :label="$t('shops.createPayment')"
              icon="i-lucide-receipt"
            />
          </div>
        </UCard>

        <!-- The links this shop keeps, next to the payments they price. -->
        <ShopCodes
          :key="shopId"
          :shop-id="shopId"
        />
      </div>

      <div
        v-show="tab === 'shop'"
        id="shop-panel-shop"
        role="tabpanel"
        aria-labelledby="shop-tab-shop"
      >
        <ShopEditor :shop-id="shopId" />
      </div>
    </div>
  </UPageSection>
</template>
