<script setup lang="ts">
import type { ShopWithProducts } from '~/composables/useShop'

// A seller may keep several shops — a café and a bakery. The profile lists
// them; each one opens on its own page, where its catalogue is edited.
const api = useShopApi()

const shops = ref<ShopWithProducts[]>([])
const loading = ref(true)
const failure = ref('')

const name = ref('')
const description = ref('')
const creating = ref(false)
const formOpen = ref(false)

const canCreate = computed(() => !creating.value && name.value.trim().length > 0)

async function load() {
  loading.value = true
  failure.value = ''
  try {
    shops.value = await api.list()
    // Nothing to list yet: the form is the page.
    formOpen.value = shops.value.length === 0
  } catch (error) {
    failure.value = apiErrorMessage(error, 'Could not load your shops.')
  } finally {
    loading.value = false
  }
}

async function createShop() {
  if (!canCreate.value) return
  creating.value = true
  failure.value = ''
  try {
    const shop = await api.createShop({
      name: name.value.trim(),
      description: description.value.trim()
    })
    // Straight to the new shop, which is empty and waiting for products.
    await navigateTo(`/shops/${shop.id}`)
  } catch (error) {
    failure.value = apiErrorMessage(error, 'Could not create the shop.')
  } finally {
    creating.value = false
  }
}

onMounted(load)
</script>

<template>
  <UCard class="min-w-0">
    <template #header>
      <div class="flex items-center justify-between gap-3">
        <div>
          <h2 class="text-base font-semibold">
            Shops
          </h2>
          <p class="text-sm text-muted">
            Your menus. Each shop keeps its own products, and a payment can be priced from them.
          </p>
        </div>
        <UBadge
          v-if="shops.length"
          color="primary"
          variant="subtle"
        >
          {{ shops.length }} {{ shops.length === 1 ? 'shop' : 'shops' }}
        </UBadge>
      </div>
    </template>

    <div class="flex flex-col gap-4">
      <UAlert
        v-if="failure"
        color="error"
        variant="subtle"
        icon="i-lucide-circle-alert"
        title="Shop error"
        :description="failure"
      />

      <p
        v-if="loading"
        class="py-4 text-center text-sm text-muted"
      >
        Reading your shops…
      </p>

      <template v-else>
        <ul
          v-if="shops.length"
          class="flex flex-col divide-y divide-default"
        >
          <li
            v-for="shop in shops"
            :key="shop.id"
            class="flex min-w-0 items-center justify-between gap-3 py-3"
          >
            <div class="min-w-0">
              <p class="text-sm font-medium break-words">
                {{ shop.name }}
              </p>
              <p
                v-if="shop.description"
                class="text-xs text-muted break-words"
              >
                {{ shop.description }}
              </p>
              <p class="mt-0.5 text-xs text-muted">
                {{ shop.products.length }}
                {{ shop.products.length === 1 ? 'product' : 'products' }}
              </p>
            </div>
            <UButton
              :to="`/shops/${shop.id}`"
              label="Open shop"
              icon="i-lucide-arrow-right"
              trailing
              color="neutral"
              variant="soft"
              size="sm"
            />
          </li>
        </ul>

        <p
          v-else
          class="text-sm text-muted"
        >
          No shops yet — name the first one below.
        </p>

        <UButton
          v-if="!formOpen"
          label="New shop"
          icon="i-lucide-plus"
          color="neutral"
          variant="soft"
          size="sm"
          class="self-start"
          @click="formOpen = true"
        />

        <div
          v-if="formOpen"
          class="flex flex-col gap-3 rounded-xl border border-dashed border-default p-4"
        >
          <div class="flex items-center gap-2">
            <UIcon
              name="i-lucide-store"
              class="text-primary size-5"
            />
            <span class="font-medium">New shop</span>
          </div>
          <UFormField label="Shop name">
            <UInput
              v-model="name"
              class="w-full"
              placeholder="Corner Shop"
              :maxlength="48"
            />
          </UFormField>
          <UFormField
            label="Short description"
            help="Optional — a line about what the shop sells."
          >
            <UInput
              v-model="description"
              class="w-full"
              placeholder="Flat whites and fresh bread"
              :maxlength="160"
            />
          </UFormField>
          <div class="flex flex-wrap items-center gap-2">
            <UButton
              label="Create shop"
              icon="i-lucide-store"
              :loading="creating"
              :disabled="!canCreate"
              @click="createShop"
            />
            <UButton
              v-if="shops.length"
              label="Cancel"
              color="neutral"
              variant="ghost"
              @click="formOpen = false"
            />
          </div>
        </div>
      </template>
    </div>
  </UCard>
</template>
