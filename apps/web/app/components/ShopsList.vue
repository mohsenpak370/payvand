<script setup lang="ts">
import type { ShopWithProducts } from '~/composables/useShop'

// A seller may keep several shops — a café and a bakery. The /shops page lists
// them; each one opens on its own page, where its catalogue is edited. The
// form lives in a modal so an empty page is not a form to fill in by accident.
const { t } = useI18n()
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
    failure.value = apiErrorMessage(error, t('shopsList.loadFailure'))
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
    failure.value = apiErrorMessage(error, t('shopsList.createFailure'))
  } finally {
    creating.value = false
  }
}

/** "1 product" / "2 products", in whatever forms the locale defines. */
function productCount(count: number) {
  return t('shopsList.products', { count }, count)
}

onMounted(load)
</script>

<template>
  <UCard class="min-w-0">
    <template #header>
      <div class="flex items-start justify-between gap-3">
        <div class="min-w-0">
          <h2 class="text-base font-semibold">
            {{ $t('shopsList.title') }}
          </h2>
          <p class="text-sm text-muted">
            {{ $t('shopsList.subtitle') }}
          </p>
        </div>
        <UButton
          :label="$t('shopsList.newShop')"
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
        :title="$t('shopsList.errorTitle')"
        :description="failure"
      />

      <p
        v-if="loading"
        class="py-4 text-center text-sm text-muted"
      >
        {{ $t('shopsList.loading') }}
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
                {{ productCount(shop.products.length) }}
              </p>
            </div>
            <UButton
              :to="`/shops/${shop.id}`"
              :label="$t('shopsList.openShop')"
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
          {{ $t('shopsList.empty') }}
        </p>
      </template>
    </div>

    <UModal
      v-model:open="formOpen"
      :title="$t('shopsList.newShop')"
      :description="$t('shopsList.modalDescription')"
      :ui="{ content: 'max-w-md' }"
    >
      <template #body>
        <div class="flex flex-col gap-4">
          <UFormField :label="$t('shopsList.nameLabel')">
            <UInput
              v-model="name"
              class="w-full"
              :placeholder="$t('shopsList.namePlaceholder')"
              :maxlength="48"
              autofocus
            />
          </UFormField>
          <UFormField
            :label="$t('shopsList.descriptionLabel')"
            :help="$t('shopsList.descriptionHelp')"
          >
            <UInput
              v-model="description"
              class="w-full"
              :placeholder="$t('shopsList.descriptionPlaceholder')"
              :maxlength="160"
            />
          </UFormField>

          <UAlert
            v-if="failure"
            color="error"
            variant="subtle"
            icon="i-lucide-circle-alert"
            :title="$t('shopsList.createFailure')"
            :description="failure"
          />
        </div>
      </template>

      <template #footer>
        <div class="flex w-full items-center justify-end gap-2">
          <UButton
            :label="$t('common.cancel')"
            color="neutral"
            variant="ghost"
            @click="formOpen = false"
          />
          <UButton
            :label="$t('shopsList.createShop')"
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
