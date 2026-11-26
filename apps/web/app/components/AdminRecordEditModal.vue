<script setup lang="ts">
import type { RecordEdit } from '~/composables/useAdminRecords'
import { parsePrice, priceField, recordCurrency } from '~/utils/adminRecords'

/**
 * Corrects a shop's name and description, or a product's name, details,
 * category and price. The limits mirror the API's, which checks them again;
 * a shop's currency is not editable, because its prices are already in it.
 */
const props = defineProps<{
  edit: RecordEdit | null
  busy: boolean
}>()

const emit = defineEmits<{
  close: []
  save: [edit: RecordEdit, body: Record<string, unknown>, done: (failure: string) => void]
}>()

const { t } = useI18n()

const open = computed({
  get: () => props.edit !== null,
  set: (value) => {
    if (!value) emit('close')
  }
})

const name = ref('')
const description = ref('')
const details = ref('')
const category = ref('')
const price = ref('')
const failure = ref('')

watch(() => props.edit, (edit) => {
  failure.value = ''
  if (!edit) return
  name.value = edit.row.name
  if (edit.kind === 'shop') {
    description.value = edit.row.description
  } else {
    details.value = edit.row.details
    category.value = edit.row.category
    price.value = priceField(edit.row.price_lamports, edit.row.currency)
  }
}, { immediate: true })

const symbol = computed(() =>
  props.edit?.kind === 'product' ? recordCurrency(props.edit.row.currency).toUpperCase() : ''
)

function submit() {
  const edit = props.edit
  if (!edit) return
  failure.value = ''
  let body: Record<string, unknown>
  if (edit.kind === 'shop') {
    body = { name: name.value, description: description.value }
  } else {
    const units = parsePrice(price.value, edit.row.currency)
    if (units === null) {
      failure.value = t('admin.records.badPrice')
      return
    }
    body = {
      name: name.value,
      details: details.value,
      category: category.value,
      price_lamports: units
    }
  }
  emit('save', edit, body, (message) => {
    if (message) failure.value = message
    else emit('close')
  })
}
</script>

<template>
  <UModal
    v-model:open="open"
    :title="edit?.kind === 'product' ? $t('admin.records.editProduct') : $t('admin.records.editShop')"
    :transition="false"
  >
    <template #body>
      <form
        id="admin-record-edit"
        class="flex flex-col gap-4"
        @submit.prevent="submit"
      >
        <UFormField
          :label="$t('admin.records.name')"
          required
        >
          <UInput
            v-model="name"
            class="w-full"
            :maxlength="edit?.kind === 'product' ? 32 : 48"
          />
        </UFormField>

        <UFormField
          v-if="edit?.kind === 'shop'"
          :label="$t('admin.records.description')"
        >
          <UTextarea
            v-model="description"
            class="w-full"
            :maxlength="160"
            autoresize
          />
        </UFormField>

        <template v-if="edit?.kind === 'product'">
          <UFormField :label="$t('admin.records.details')">
            <UTextarea
              v-model="details"
              class="w-full"
              :maxlength="160"
              autoresize
            />
          </UFormField>
          <UFormField :label="$t('admin.records.category')">
            <UInput
              v-model="category"
              class="w-full"
              :maxlength="32"
            />
          </UFormField>
          <UFormField
            :label="$t('admin.records.price')"
            :hint="symbol"
            required
          >
            <UInput
              v-model="price"
              class="w-full"
              inputmode="decimal"
            />
          </UFormField>
        </template>

        <UAlert
          v-if="failure"
          color="error"
          variant="subtle"
          icon="i-lucide-circle-alert"
          :description="failure"
        />
      </form>
    </template>
    <template #footer>
      <div class="flex w-full justify-end gap-2">
        <UButton
          :label="$t('admin.records.cancel')"
          color="neutral"
          variant="soft"
          @click="emit('close')"
        />
        <UButton
          type="submit"
          form="admin-record-edit"
          :label="$t('admin.records.save')"
          :loading="busy"
        />
      </div>
    </template>
  </UModal>
</template>
