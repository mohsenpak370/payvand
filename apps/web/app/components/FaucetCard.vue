<script setup lang="ts">
// One claim per account, to the linked wallet: enough SOL for fees and enough
// USDC/EURC to pay a few invoices, so a first visit never stops at an empty
// balance. The links are the way to get more once that is spent.

const { user } = useAuth()
const faucet = useFaucet()

const sol = computed(() => (faucet.status.value?.sol_lamports ?? 0) / 1_000_000_000)
const tokens = computed(() => (faucet.status.value?.token_base_units ?? 0) / 1_000_000)

/** "1 SOL + 10 USDC + 10 EURC", in the numbers the API will actually send. */
const amounts = computed(() => `${sol.value} SOL + ${tokens.value} USDC + ${tokens.value} EURC`)

onMounted(() => {
  if (user.value) void faucet.load()
})
</script>

<template>
  <UCard class="min-w-0">
    <div class="flex flex-col gap-4">
      <div class="flex items-start gap-4">
        <div class="bg-primary/10 flex size-10 shrink-0 items-center justify-center rounded-full">
          <UIcon
            name="i-lucide-droplets"
            class="text-primary size-5"
          />
        </div>
        <div class="min-w-0">
          <h3 class="text-base font-semibold">
            {{ $t('start.faucet.title') }}
          </h3>
          <p class="text-muted mt-1 text-sm leading-relaxed">
            {{ $t('start.faucet.body') }}
          </p>
        </div>
      </div>

      <UAlert
        v-if="faucet.failure.value"
        color="error"
        variant="subtle"
        icon="i-lucide-circle-alert"
        :description="faucet.failure.value"
      />

      <!-- Not signed in: the tokens belong to an account, because the claim is
           recorded against it. -->
      <div
        v-if="!user"
        class="flex flex-wrap items-center gap-2"
      >
        <UButton
          to="/signin"
          :label="$t('app.auth.signIn')"
          icon="i-lucide-log-in"
          size="sm"
        />
        <span class="text-muted text-sm">{{ $t('start.faucet.signedOut') }}</span>
      </div>

      <template v-else>
        <p
          v-if="faucet.loading.value && !faucet.status.value"
          class="text-muted text-sm"
        >
          {{ $t('start.faucet.loading') }}
        </p>

        <template v-else-if="faucet.status.value">
          <UAlert
            v-if="faucet.status.value.claimed"
            color="success"
            variant="subtle"
            icon="i-lucide-circle-check"
            :title="$t('start.faucet.claimedTitle')"
            :description="$t('start.faucet.claimedBody')"
          />

          <div
            v-else-if="faucet.status.value.claimable"
            class="flex flex-col gap-2 sm:flex-row sm:items-center"
          >
            <UButton
              :label="$t('start.faucet.claim', { amounts })"
              icon="i-lucide-download"
              :loading="faucet.claiming.value"
              @click="faucet.claim()"
            />
            <p class="text-muted text-xs">
              {{ $t('start.faucet.toWallet') }}
            </p>
          </div>

          <div
            v-else
            class="flex flex-col gap-2"
          >
            <p class="text-muted text-sm">
              {{ faucet.status.value.reason }}
            </p>
            <UButton
              v-if="!faucet.status.value.wallet"
              to="/profile"
              :label="$t('start.faucet.linkWallet')"
              icon="i-lucide-wallet"
              color="neutral"
              variant="soft"
              size="sm"
              class="self-start"
            />
          </div>
        </template>
      </template>

      <!-- The claim is one per account, so someone who already got theirs is
           pointed at the links that are not: the official faucets. -->
      <p class="border-default border-t pt-3 text-xs leading-relaxed text-muted">
        {{ faucet.status.value?.claimed ? $t('start.faucet.claimedMore') : $t('start.faucet.more') }}
        <ULink
          to="https://faucet.solana.com/"
          target="_blank"
          rel="noopener noreferrer"
          class="underline underline-offset-4"
        >faucet.solana.com</ULink>
        <span aria-hidden="true"> · </span>
        <ULink
          to="https://faucet.circle.com/"
          target="_blank"
          rel="noopener noreferrer"
          class="underline underline-offset-4"
        >faucet.circle.com</ULink>
      </p>
    </div>
  </UCard>
</template>
