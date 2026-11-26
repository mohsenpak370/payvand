<script setup lang="ts">
import type { AdminCode, AdminProduct, AdminShop, RecordPage } from '~/composables/useAdminApi'
import { RECORD_KINDS, type RecordEdit, type RecordKind } from '~/composables/useAdminRecords'
import { adminDate } from '~/utils/adminFormat'
import { recordPrice } from '~/utils/adminRecords'

/**
 * Shops, products and discount codes, one table at a time: search, page,
 * correct and delete. The search draft belongs to each table, so switching
 * tables never carries one table's query into another.
 */
const props = defineProps<{
  kind: RecordKind
  shops: RecordPage<AdminShop> | null
  products: RecordPage<AdminProduct> | null
  codes: RecordPage<AdminCode> | null
  searches: Record<RecordKind, string>
  loading: boolean
  failure: string
  busy: boolean
}>()

const emit = defineEmits<{
  show: [kind: RecordKind]
  search: [query: string]
  previous: []
  next: []
  edit: [edit: RecordEdit]
  remove: [kind: RecordKind, id: string, label: string]
}>()

const { t } = useI18n()

const ICONS: Record<RecordKind, string> = {
  shops: 'i-lucide-store',
  products: 'i-lucide-package',
  codes: 'i-lucide-ticket-percent'
}

const kinds = computed(() =>
  RECORD_KINDS.map(value => ({ value, label: t(`admin.records.kinds.${value}`), icon: ICONS[value] }))
)

const selected = computed<RecordKind>({
  get: () => props.kind,
  set: kind => emit('show', kind)
})

const query = ref(props.searches[props.kind])
watch(() => props.kind, (kind) => {
  query.value = props.searches[kind]
})

const page = computed<RecordPage<unknown> | null>(() =>
  props.kind === 'shops' ? props.shops : props.kind === 'products' ? props.products : props.codes
)
const pageFrom = computed(() => (page.value?.total ? page.value.offset + 1 : 0))
const pageTo = computed(() =>
  page.value ? Math.min(page.value.offset + page.value.limit, page.value.total) : 0
)
</script>

<template>
  <UCard class="min-w-0">
    <template #header>
      <div class="flex flex-col gap-3">
        <div class="overflow-x-auto">
          <UTabs
            v-model="selected"
            :items="kinds"
            :content="false"
            size="sm"
            class="min-w-max"
          />
        </div>
        <form
          class="flex gap-2"
          @submit.prevent="emit('search', query)"
        >
          <UInput
            v-model="query"
            class="min-w-0 flex-1"
            :placeholder="$t(`admin.records.searchPlaceholder.${kind}`)"
            icon="i-lucide-search"
          />
          <UButton
            type="submit"
            :label="$t('admin.users.search')"
            :loading="loading"
          />
        </form>
      </div>
    </template>

    <UAlert
      v-if="failure"
      class="mb-4"
      color="error"
      variant="subtle"
      icon="i-lucide-circle-alert"
      :title="$t('admin.users.errorTitle')"
      :description="failure"
    />

    <p
      v-if="loading && !page"
      class="py-6 text-center text-sm text-muted"
    >
      {{ $t('common.loading') }}
    </p>
    <p
      v-else-if="page && page.items.length === 0"
      class="py-6 text-center text-sm text-muted"
    >
      {{ $t('admin.records.empty') }}
    </p>

    <div
      v-else-if="kind === 'shops' && shops"
      class="overflow-x-auto"
    >
      <table class="w-full min-w-[40rem] text-sm">
        <thead>
          <tr class="border-b text-start text-muted">
            <th class="py-2 pe-3 font-medium">
              {{ $t('admin.records.shop') }}
            </th>
            <th class="py-2 pe-3 font-medium">
              {{ $t('admin.records.owner') }}
            </th>
            <th class="py-2 pe-3 font-medium">
              {{ $t('admin.records.currency') }}
            </th>
            <th class="py-2 pe-3 text-right font-medium">
              {{ $t('admin.records.productCount') }}
            </th>
            <th class="py-2 pe-3 font-medium">
              {{ $t('admin.records.created') }}
            </th>
            <th class="py-2 text-right font-medium">
              {{ $t('admin.users.actions') }}
            </th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="shop in shops.items"
            :key="shop.id"
            class="border-b align-top last:border-0"
          >
            <td class="max-w-[16rem] py-2 pe-3">
              <p class="truncate font-medium">
                {{ shop.name }}
              </p>
              <p class="truncate text-xs text-muted">
                {{ shop.description || '—' }}
              </p>
            </td>
            <td class="max-w-[12rem] truncate py-2 pe-3">
              {{ shop.owner }}
            </td>
            <td class="py-2 pe-3 uppercase">
              {{ shop.currency }}
            </td>
            <td class="py-2 pe-3 text-right">
              {{ shop.products }}
            </td>
            <td class="py-2 pe-3 whitespace-nowrap text-muted">
              {{ adminDate(shop.created_at) }}
            </td>
            <td class="py-2 text-right whitespace-nowrap">
              <AdminRecordActions
                :busy="busy"
                editable
                @edit="emit('edit', { kind: 'shop', row: shop })"
                @remove="emit('remove', 'shops', shop.id, shop.name)"
              />
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <div
      v-else-if="kind === 'products' && products"
      class="overflow-x-auto"
    >
      <table class="w-full min-w-[44rem] text-sm">
        <thead>
          <tr class="border-b text-start text-muted">
            <th class="py-2 pe-3 font-medium">
              {{ $t('admin.records.product') }}
            </th>
            <th class="py-2 pe-3 font-medium">
              {{ $t('admin.records.shop') }}
            </th>
            <th class="py-2 pe-3 font-medium">
              {{ $t('admin.records.category') }}
            </th>
            <th class="py-2 pe-3 text-right font-medium">
              {{ $t('admin.records.price') }}
            </th>
            <th class="py-2 pe-3 font-medium">
              {{ $t('admin.records.owner') }}
            </th>
            <th class="py-2 text-right font-medium">
              {{ $t('admin.users.actions') }}
            </th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="product in products.items"
            :key="product.id"
            class="border-b align-top last:border-0"
          >
            <td class="max-w-[14rem] py-2 pe-3">
              <p class="truncate font-medium">
                {{ product.name }}
              </p>
              <p class="truncate text-xs text-muted">
                {{ product.details || '—' }}
              </p>
            </td>
            <td class="max-w-[10rem] truncate py-2 pe-3">
              {{ product.shop }}
            </td>
            <td class="max-w-[8rem] truncate py-2 pe-3">
              {{ product.category || '—' }}
            </td>
            <td class="py-2 pe-3 text-right whitespace-nowrap">
              {{ recordPrice(product.price_lamports, product.currency) }}
            </td>
            <td class="max-w-[12rem] truncate py-2 pe-3">
              {{ product.owner }}
            </td>
            <td class="py-2 text-right whitespace-nowrap">
              <AdminRecordActions
                :busy="busy"
                editable
                @edit="emit('edit', { kind: 'product', row: product })"
                @remove="emit('remove', 'products', product.id, product.name)"
              />
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <div
      v-else-if="kind === 'codes' && codes"
      class="overflow-x-auto"
    >
      <table class="w-full min-w-[32rem] text-sm">
        <thead>
          <tr class="border-b text-start text-muted">
            <th class="py-2 pe-3 font-medium">
              {{ $t('admin.records.code') }}
            </th>
            <th class="py-2 pe-3 text-right font-medium">
              {{ $t('admin.records.percentOff') }}
            </th>
            <th class="py-2 pe-3 font-medium">
              {{ $t('admin.records.owner') }}
            </th>
            <th class="py-2 pe-3 font-medium">
              {{ $t('admin.records.created') }}
            </th>
            <th class="py-2 text-right font-medium">
              {{ $t('admin.users.actions') }}
            </th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="code in codes.items"
            :key="code.id"
            class="border-b last:border-0"
          >
            <td class="py-2 pe-3 font-mono text-xs">
              {{ code.code }}
            </td>
            <td class="py-2 pe-3 text-right">
              {{ code.percent_off }}%
            </td>
            <td class="max-w-[14rem] truncate py-2 pe-3">
              {{ code.owner }}
            </td>
            <td class="py-2 pe-3 whitespace-nowrap text-muted">
              {{ adminDate(code.created_at) }}
            </td>
            <td class="py-2 text-right whitespace-nowrap">
              <AdminRecordActions
                :busy="busy"
                @remove="emit('remove', 'codes', code.id, code.code)"
              />
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <div
      v-if="page && page.total > page.limit"
      class="mt-4 flex flex-wrap items-center justify-between gap-3 text-sm text-muted"
    >
      <span>{{ $t('admin.records.range', { from: pageFrom, to: pageTo, total: page.total }) }}</span>
      <div class="flex gap-2">
        <UButton
          :label="$t('admin.users.previous')"
          color="neutral"
          variant="soft"
          size="sm"
          :disabled="page.offset === 0 || loading"
          @click="emit('previous')"
        />
        <UButton
          :label="$t('admin.users.next')"
          color="neutral"
          variant="soft"
          size="sm"
          :disabled="pageTo >= page.total || loading"
          @click="emit('next')"
        />
      </div>
    </div>
  </UCard>
</template>
