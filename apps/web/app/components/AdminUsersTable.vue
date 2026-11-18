<script setup lang="ts">
import type { AdminUser } from '~/composables/useAdminApi'
import { adminDate } from '~/utils/adminFormat'

defineProps<{
  rows: AdminUser[]
  busy: boolean
}>()

const emit = defineEmits<{ delete: [row: AdminUser] }>()
</script>

<template>
  <!--
    The table keeps a readable minimum width and scrolls sideways inside the
    card, so its columns never stretch the page on a phone.
  -->
  <div class="overflow-x-auto">
    <table class="w-full min-w-[32rem] text-sm">
      <thead>
        <tr class="border-b text-start text-muted">
          <th class="py-2 pe-3 font-medium">
            {{ $t('admin.users.account') }}
          </th>
          <th class="py-2 pe-3 font-medium">
            {{ $t('admin.users.signedUp') }}
          </th>
          <th class="py-2 pe-3 text-right font-medium">
            {{ $t('admin.users.sessions') }}
          </th>
          <th class="py-2 pe-3 text-right font-medium">
            {{ $t('admin.users.codes') }}
          </th>
          <th class="py-2 text-right font-medium">
            {{ $t('admin.users.actions') }}
          </th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="row in rows"
          :key="row.id"
          class="border-b last:border-0"
        >
          <td class="py-2 pe-3">
            <div>{{ row.email }}</div>
            <div
              v-if="row.wallet"
              class="text-muted text-xs"
            >
              {{ row.wallet }}
            </div>
          </td>
          <td class="py-2 pe-3 whitespace-nowrap text-muted">
            {{ adminDate(row.created_at) }}
          </td>
          <td class="py-2 pe-3 text-right">
            {{ row.sessions }}
          </td>
          <td class="py-2 pe-3 text-right">
            {{ row.codes }}
          </td>
          <td class="py-2 text-right">
            <UButton
              icon="i-lucide-trash-2"
              color="error"
              variant="subtle"
              size="xs"
              :label="$t('admin.users.delete')"
              :loading="busy"
              @click="emit('delete', row)"
            />
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
