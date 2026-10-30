<script setup lang="ts">
import type { BudgetState } from '~/composables/useBudget'

useSeoMeta({ title: 'Budget — Payvand' })

definePageMeta({ middleware: 'auth' })

const { client } = useSolanaClient()
const { publicKey, connected } = useSolanaWallet()
const { execute, loading: txLoading } = useSolanaSignAndSendTransaction()
const budgetApi = useBudget()

const limitSol = ref('2')
const busy = ref(false)
const failure = ref('')
const success = ref('')
const budget = ref<BudgetState | null>(null)
const budgetPot = ref(0)

const remaining = computed(() => (budget.value ? budgetApi.remainingThisMonth(budget.value) : 0))

function shorten(value: string) {
  return `${value.slice(0, 4)}…${value.slice(-4)}`
}

async function refresh() {
  budget.value = null
  budgetPot.value = 0
  if (!publicKey.value) return
  try {
    const loaded = await budgetApi.loadBudget(publicKey.value)
    budget.value = loaded
    if (loaded) budgetPot.value = await budgetApi.availablePot(loaded)
  } catch (error) {
    failure.value = error instanceof Error ? error.message : 'Could not read your budget.'
  }
}

watch(publicKey, refresh, { immediate: true })

/**
 * The one signature this feature ever asks for: funds the pot to the limit,
 * tops up the session key's fee float, and registers (or rotates) the
 * session key that will pay silently afterwards.
 */
async function saveLimit() {
  const buyer = publicKey.value
  if (!buyer) return

  failure.value = ''
  success.value = ''
  const limit = solToLamports(Number(limitSol.value))
  if (!Number.isFinite(limit) || limit <= 0) {
    failure.value = 'Enter a monthly limit greater than 0.'
    return
  }

  busy.value = true
  try {
    const session = await ensureSessionSigner(buyer)
    const instruction = buildInstruction(
      'open_budget',
      { monthly_limit: limit },
      { buyer, budget: await deriveBudgetPda(buyer), session_key: session.address }
    )
    const wire = await compileWireTransaction({ instruction, payer: buyer, rpc: client.rpc })
    await execute(wire, {
      confirm: true,
      confirmation: { commitment: 'confirmed', timeoutMs: 120_000 }
    })
    success.value = budget.value
      ? 'Budget updated — your limit and pot are in sync again.'
      : 'Budget enabled. Payments inside it now need no wallet approval at all.'
    await refresh()
  } catch (error) {
    failure.value = error instanceof Error ? error.message : 'The wallet rejected the request.'
  } finally {
    busy.value = false
  }
}

/** Reclaim every remaining lamport and retire the session key (1 signature). */
async function closeBudget() {
  const buyer = publicKey.value
  if (!buyer) return

  failure.value = ''
  success.value = ''
  busy.value = true
  try {
    const instruction = buildInstruction(
      'close_budget',
      {},
      { budget: await deriveBudgetPda(buyer), buyer }
    )
    const wire = await compileWireTransaction({ instruction, payer: buyer, rpc: client.rpc })
    await execute(wire, {
      confirm: true,
      confirmation: { commitment: 'confirmed', timeoutMs: 120_000 }
    })
    forgetSessionSigner(buyer)
    success.value = 'Budget closed — everything left went back to your wallet.'
    await refresh()
  } catch (error) {
    failure.value = error instanceof Error ? error.message : 'Could not close the budget.'
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <UPageSection
    title="Monthly spending budget"
    description="Set your limit once a month — your wallet signs only that one time. After that, payments inside the limit go through instantly, with no approval popups."
  >
    <UCard>
      <template #header>
        <div class="flex items-center justify-between gap-3">
          <h2 class="text-base font-semibold">
            Your budget
          </h2>
          <WalletConnect />
        </div>
      </template>

      <div class="flex flex-col gap-4">
        <template v-if="connected && budget">
          <div class="grid grid-cols-2 gap-4 sm:grid-cols-4">
            <div>
              <p class="text-sm text-muted">
                Monthly limit
              </p>
              <p class="text-lg font-semibold">
                {{ lamportsToSol(budget.monthlyLimit) }} SOL
              </p>
            </div>
            <div>
              <p class="text-sm text-muted">
                Left this month
              </p>
              <p class="text-lg font-semibold text-success">
                {{ lamportsToSol(remaining) }} SOL
              </p>
            </div>
            <div>
              <p class="text-sm text-muted">
                Pot (leftover included)
              </p>
              <p class="text-lg font-semibold">
                {{ lamportsToSol(budgetPot) }} SOL
              </p>
            </div>
            <div>
              <p class="text-sm text-muted">
                Session key
              </p>
              <p class="font-mono text-sm">
                {{ shorten(budget.sessionKey) }}
              </p>
            </div>
          </div>

          <p class="text-sm text-muted">
            The limit resets on the 1st of each month. The pot keeps whatever you did not
            spend, and every lamport is still yours: closing the budget returns all of it.
          </p>

          <UFormField
            label="Monthly limit (SOL)"
            help="Applied on the next signature — top up, lower, or raise it any time."
          >
            <UInput
              v-model="limitSol"
              type="number"
              min="0"
              step="0.1"
              class="w-full"
              placeholder="2"
            />
          </UFormField>

          <div class="flex flex-wrap gap-2">
            <UButton
              label="Save limit / top up"
              icon="i-lucide-refresh-cw"
              :loading="busy || txLoading"
              @click="saveLimit"
            />
            <UButton
              label="Close budget & reclaim funds"
              color="error"
              variant="soft"
              icon="i-lucide-trash-2"
              :loading="busy || txLoading"
              @click="closeBudget"
            />
          </div>
        </template>

        <template v-else-if="connected">
          <UFormField label="Monthly limit (SOL)">
            <UInput
              v-model="limitSol"
              type="number"
              min="0"
              step="0.1"
              class="w-full"
              placeholder="2"
            />
          </UFormField>

          <UButton
            label="Enable monthly budget"
            icon="i-lucide-wallet"
            :loading="busy || txLoading"
            @click="saveLimit"
          />

          <p class="text-sm text-muted">
            This is the only wallet approval the feature needs: it funds your pot for the
            month and creates the session key that signs future payments on this device.
          </p>
        </template>

        <p
          v-else
          class="text-sm text-muted"
        >
          Connect your wallet to see your budget.
        </p>

        <UAlert
          v-if="failure"
          color="error"
          variant="subtle"
          icon="i-lucide-circle-alert"
          title="Something went wrong"
          :description="failure"
        />
        <UAlert
          v-if="success"
          color="success"
          variant="subtle"
          icon="i-lucide-circle-check"
          title="Done"
          :description="success"
        />
      </div>
    </UCard>
  </UPageSection>
</template>
