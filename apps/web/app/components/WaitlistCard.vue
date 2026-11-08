<script setup lang="ts">
// The first thing a seller should learn is that the money is test money, and
// the second is how to hear when it is not. This card is both: a plain
// statement of what Payvand is today, and a waiting list for taking real
// payments with it later.
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
      'Could not add you to the list. Try again in a moment.'
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
          This is a test build
        </h2>
        <p class="text-muted mt-1 text-sm leading-relaxed">
          Payvand is a proof of concept. Codes, scanning and receipts all work,
          but every payment runs on a Solana test network with free test SOL —
          no real money moves.
        </p>
        <p class="text-muted mt-2 text-sm leading-relaxed">
          Want to take real payments? Leave your email and we'll tell you when
          Payvand is ready for a live wallet.
        </p>

        <UAlert
          v-if="failure"
          color="error"
          variant="subtle"
          icon="i-lucide-circle-alert"
          title="Waiting list"
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
            placeholder="you@example.com"
            class="w-full"
            :disabled="busy"
          />
          <UButton
            type="submit"
            :loading="busy"
            :disabled="!canJoin"
            class="justify-center sm:shrink-0"
            label="Join the waiting list"
          />
        </form>

        <UAlert
          v-else
          color="success"
          variant="subtle"
          icon="i-lucide-circle-check"
          title="You're on the list."
          description="We'll email you when Payvand can take real payments."
          class="mt-4"
        />
      </div>
    </div>
  </UCard>
</template>
