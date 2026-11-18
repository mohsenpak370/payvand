<script setup lang="ts">
const { t } = useI18n()

useSeoMeta({ title: t('admin.metaTitle') })
definePageMeta({ middleware: 'auth' })

const {
  stats,
  rows,
  total,
  pageSize,
  offset,
  usersLoading,
  usersFailure,
  forbidden,
  searchUsers,
  pageDown,
  pageUp,
  removeUser,
  feed,
  feedLoading,
  feedFailure,
  loadFeed,
  waitlist,
  waitlistLoading,
  waitlistFailure,
  loadWaitlist,
  removeEntry,
  busy
} = useAdminPage()
</script>

<template>
  <UPageSection
    :title="$t('admin.title')"
    :description="$t('admin.description')"
  >
    <!--
      `min-w-0` on this column and on the cards below: in a flex column each
      item defaults to `min-width: auto`, so a table's minimum width is
      otherwise pushed up through every card and widens the page instead of
      scrolling inside it.
    -->
    <div class="mx-auto flex w-full max-w-4xl min-w-0 flex-col gap-6">
      <UAlert
        v-if="forbidden"
        color="error"
        variant="subtle"
        icon="i-lucide-shield-x"
        :title="$t('admin.forbiddenTitle')"
        :description="$t('admin.forbiddenDescription')"
      />

      <template v-else>
        <AdminStatsCards :stats="stats" />

        <AdminWaitlistCard
          :waitlist="waitlist"
          :loading="waitlistLoading"
          :failure="waitlistFailure"
          :busy="busy"
          @refresh="loadWaitlist"
          @remove="removeEntry"
        />

        <AdminPaymentsCard
          :feed="feed"
          :loading="feedLoading"
          :failure="feedFailure"
          @refresh="loadFeed"
        />

        <AdminUsersCard
          :rows="rows"
          :total="total"
          :page-size="pageSize"
          :offset="offset"
          :loading="usersLoading"
          :failure="usersFailure"
          :busy="busy"
          @search="searchUsers"
          @delete="removeUser"
          @previous="pageDown"
          @next="pageUp"
        />
      </template>
    </div>
  </UPageSection>
</template>
