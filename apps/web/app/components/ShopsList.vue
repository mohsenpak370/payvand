<script setup lang="ts">
import type { ShopWithProducts } from '~/composables/useShop'

// A seller may keep several shops — a café and a bakery. The /shops page lists
// them; each one opens on its own page, where its catalogue is edited. The
// form lives in a modal so an empty page is not a form to fill in by accident.
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
  } catch (error) {
    failure.value = apiErrorMessage(error, 'Could not load your shops.')
  } finally {
    loading.value = false
  }
}

function openForm() {
  failure.value = ''
  formOpen.value = true
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
    formOpen.value = false
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
      <div class="flex items-start justify-between gap-3">
        <div class="min-w-0">
          <h2 class="text-base font-semibold">
            Your shops
          </h2>
          <p class="text-sm text-muted">
            Create and save the products you sell, then use them to create payments.
          </p>
        </div>
        <UButton
          label="New shop"
          icon="i-lucide-plus"
          size="sm"
          class="shrink-0"
          @click="openForm"
        />
      </div>
    </template>

    <div class="flex flex-col gap-4">
      <UAlert
        v-if="failure && !formOpen"
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
              <!-- One line even on a narrow phone: "2 products" is a count, not
                   a phrase worth wrapping. -->
              <p class="mt-0.5 text-xs whitespace-nowrap text-muted">
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
          No shops yet — create one to save your first products.
        </p>
      </template>
    </div>

    <UModal
      v-model:open="formOpen"
      title="New shop"
      description="A shop holds the products you sell. You can add products once it exists."
      :ui="{ content: 'max-w-md' }"
    >
      <template #body>
        <div class="flex flex-col gap-4">
          <UFormField label="Shop name">
            <UInput
              v-model="name"
              class="w-full"
              placeholder="Corner Shop"
              :maxlength="48"
              autofocus
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

          <UAlert
            v-if="failure"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            title="Could not create the shop"
            :description="failure"
          />
        </div>
      </template>

      <template #footer>
        <div class="flex w-full items-center justify-end gap-2">
          <UButton
            label="Cancel"
            color="neutral"
            variant="ghost"
            @click="formOpen = false"
          />
          <UButton
            label="Create shop"
            icon="i-lucide-store"
            :loading="creating"
            :disabled="!canCreate"
            @click="createShop"
          />
        </div>
      </template>
    </UModal>
  </UCard>
</template>
