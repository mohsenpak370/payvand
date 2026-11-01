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
const failureTitle = ref('Something went wrong')
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
 * vue-solana wallet errors carry a string code (`USER_REJECTED`,
 * `RPC_FAILURE`, …) — the same source pay.vue reads `txError?.code` from.
 */
function solanaErrorCode(error: unknown): string | undefined {
  if (typeof error !== 'object' || error === null || !('code' in error)) return undefined
  const { code } = error as { code: unknown }
  return typeof code === 'string' ? code : undefined
}

/**
 * Turns whatever went wrong into something a person can act on. The raw
 * wallet error for a doomed transaction is a bare "Internal error" — the
 * whole reason this flow pre-flights before the popup now.
 */
function describeFailure(error: unknown): string {
  switch (solanaErrorCode(error)) {
    case 'USER_REJECTED':
      return 'You rejected the request in your wallet.'
    case 'RPC_FAILURE':
      return 'Your wallet could not submit the transaction. If it warned that it could not simulate '
        + 'the transaction, the wallet most likely lacks SOL for this limit — check its balance, then try again.'
    case 'TRANSACTION_TIMEOUT':
      return 'The network is taking longer than usual to confirm. Check your wallet activity before retrying.'
    default:
      return error instanceof Error && error.message ? error.message : 'The wallet rejected the request.'
  }
}

function recordFailure(error: unknown) {
  failureTitle.value = solanaErrorCode(error) ?? 'Transaction not sent'
  failure.value = describeFailure(error)
}

/**
 * The one signature this feature ever asks for: funds the pot to the limit,
 * tops up the session key's fee float, and registers (or rotates) the
 * session key that will pay silently afterwards.
 */
async function saveLimit() {
  const buyer = publicKey.value
  if (!buyer) return

  failure.value = ''
  failureTitle.value = 'Something went wrong'
  success.value = ''
  const limit = solToLamports(Number(limitSol.value))
  if (!Number.isFinite(limit) || limit <= 0) {
    failure.value = 'Enter a monthly limit greater than 0.'
    return
  }

  busy.value = true
  try {
    const session = await ensureSessionSigner(buyer)

    // 1. Would this wallet actually be able to fund the signature? A wallet
    //    that cannot cover it reports a useless "cannot simulate" warning
    //    and a bare "Internal error" after approval — so say what is
    //    missing while we can still show numbers.
    const cost = await budgetApi.openBudgetCost(buyer, limit, session.address)
    const balance = Number(
      (await client.rpc.getBalance(buyer, { commitment: 'confirmed' }).send()).value
    )
    if (balance < cost.total) {
      failureTitle.value = 'Not enough SOL'
      failure.value
        = `This wallet holds ${lamportsToSol(balance)} SOL, but enabling a ${lamportsToSol(limit)} SOL budget needs about ${lamportsToSol(cost.total)} SOL in one signature `
          + `(${lamportsToSol(cost.topUp)} into the pot, ${lamportsToSol(cost.feeFloat)} session-key fee float, `
          + `${lamportsToSol(cost.rent)} account rent, ${lamportsToSol(cost.fee)} network fee). `
          + 'Lower the limit or top up the wallet — on devnet, https://faucet.solana.com sends free test SOL.'
      return
    }

    // 2. Build it with a memo so the wallet and explorers can describe it,
    //    then simulate it before the popup: a transaction that reaches the
    //    wallet is one that will actually succeed.
    const instruction = buildInstruction(
      'open_budget',
      { monthly_limit: limit },
      { buyer, budget: await deriveBudgetPda(buyer), session_key: session.address }
    )
    const memo = buildMemoInstruction(
      budget.value
        ? `Payvand: top up monthly budget to ${lamportsToSol(limit)} SOL`
        : `Payvand: enable monthly budget of ${lamportsToSol(limit)} SOL`,
      buyer
    )
    const wire = await compileWireTransaction({
      instruction: [instruction, memo],
      payer: buyer,
      rpc: client.rpc
    })
    await budgetApi.preflight(wire)

    await execute(wire, {
      confirm: true,
      confirmation: { commitment: 'confirmed', timeoutMs: 120_000 }
    })
    success.value = budget.value
      ? 'Budget updated — your limit and pot are in sync again.'
      : 'Budget enabled. Payments inside it now need no wallet approval at all.'
    await refresh()
  } catch (error) {
    recordFailure(error)
  } finally {
    busy.value = false
  }
}

/** Reclaim every remaining lamport and retire the session key (1 signature). */
async function closeBudget() {
  const buyer = publicKey.value
  if (!buyer) return

  failure.value = ''
  failureTitle.value = 'Something went wrong'
  success.value = ''
  if (!budget.value) {
    failure.value = 'There is no budget to close yet.'
    return
  }
  busy.value = true
  try {
    const instruction = buildInstruction(
      'close_budget',
      {},
      { budget: await deriveBudgetPda(buyer), buyer }
    )
    const memo = buildMemoInstruction('Payvand: close budget and reclaim all funds', buyer)
    const wire = await compileWireTransaction({
      instruction: [instruction, memo],
      payer: buyer,
      rpc: client.rpc
    })
    await budgetApi.preflight(wire)
    await execute(wire, {
      confirm: true,
      confirmation: { commitment: 'confirmed', timeoutMs: 120_000 }
    })
    forgetSessionSigner(buyer)
    success.value = 'Budget closed — everything left went back to your wallet.'
    await refresh()
  } catch (error) {
    recordFailure(error)
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
          :title="failureTitle"
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
