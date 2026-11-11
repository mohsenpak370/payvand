<script setup lang="ts">
// The first thing a seller should learn is that the money is test money, and
// the second is how to hear when it is not. This card is both: a plain
// statement of what Payvand is today, and a waiting list for taking real
// payments with it later.
const { t } = useI18n()
const config = useRuntimeConfig()

const email = ref('')
const busy = ref(false)
const joined = ref(false)
const failure = ref('')

const canJoin = computed(() => !busy.value && email.value.trim().length > 0)

async function join() {
  if (!canJoin.value) return
  busy.value = true
  failure.value = ''
  try {
    await $fetch(`${config.public.apiBase}/api/waitlist`, {
      method: 'POST',
      body: { email: email.value.trim() }
    })
    // The API answers the same for an address already on the list, so the
    // seller is never told they are second.
    joined.value = true
    email.value = ''
  } catch (error) {
    failure.value = apiErrorMessage(
      error,
      t('waitlist.joinFailure')
    )
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <UCard>
    <div class="flex flex-col gap-4 sm:flex-row sm:items-start sm:gap-5">
      <div
        class="bg-elevated flex size-10 shrink-0 items-center justify-center rounded-full"
      >
        <UIcon
          name="i-lucide-flask-conical"
          class="text-primary size-5"
        />
      </div>

      <div class="min-w-0 flex-1">
        <h2 class="text-base font-semibold">
          {{ $t('waitlist.title') }}
        </h2>
        <p class="text-muted mt-1 text-sm leading-relaxed">
          {{ $t('waitlist.body') }}
        </p>
        <p class="text-muted mt-2 text-sm leading-relaxed">
          {{ $t('waitlist.invite') }}
        </p>

        <UAlert
          v-if="failure"
          color="error"
          variant="subtle"
          icon="i-lucide-circle-alert"
          :title="$t('waitlist.alertTitle')"
          :description="failure"
          class="mt-4"
        />

        <form
          v-if="!joined"
          class="mt-4 flex flex-col gap-2 sm:flex-row"
          @submit.prevent="join"
        >
          <UInput
            v-model="email"
            type="email"
            name="email"
            autocomplete="email"
            :placeholder="$t('waitlist.placeholder')"
            class="w-full"
            :disabled="busy"
          />
          <UButton
            type="submit"
            :loading="busy"
            :disabled="!canJoin"
            class="justify-center sm:shrink-0"
            :label="$t('waitlist.join')"
          />
        </form>

        <UAlert
          v-else
          color="success"
          variant="subtle"
          icon="i-lucide-circle-check"
          :title="$t('waitlist.successTitle')"
          :description="$t('waitlist.successBody')"
          class="mt-4"
        />
      </div>
    </div>
  </UCard>
</template>
