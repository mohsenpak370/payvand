<script setup lang="ts">
import type { BudgetState, TokenBudgetState } from '~/composables/useBudget'
import type { SupportedCurrency } from '~/composables/usePaymentApi'
import { currencySymbol, formatTokenAmount } from '~/composables/usePaymentApi'
import { currenciesForCluster, mintForCurrency } from '~/composables/usePaymentProgram'

useSeoMeta({ title: 'Budget — Payvand' })

definePageMeta({ middleware: 'auth' })

const { client } = useSolanaClient()
const { publicKey, connected } = useSolanaWallet()
const { execute, loading: txLoading } = useSolanaSignAndSendTransaction()
const budgetApi = useBudget()

// One allowance per currency: SOL holds lamports, each stablecoin holds its
// own tokens. The card follows whichever currency is selected.
const cluster = useRuntimeConfig().public.cluster as string
const currencies = currenciesForCluster(cluster)
const activeCurrency = ref<SupportedCurrency>('sol')
const limitAmount = ref('2')
const tokenBudgets = ref<Partial<Record<SupportedCurrency, TokenBudgetState | null>>>({})

const busy = ref(false)
const failure = ref('')
const failureTitle = ref('Something went wrong')
const success = ref('')
const budget = ref<BudgetState | null>(null)
const budgetPot = ref(0)

const activeMint = computed(() =>
  activeCurrency.value === 'sol'
    ? null
    : mintForCurrency(activeCurrency.value as 'usdc' | 'eurc', cluster)
)

const tokenBudget = computed(() => tokenBudgets.value[activeCurrency.value] ?? null)

/** The allowance on screen, whichever currency it is held in. */
const view = computed(() => {
  if (activeCurrency.value === 'sol') {
    if (!budget.value) return null
    return {
      limit: budget.value.monthlyLimit,
      remaining: budgetApi.remainingThisMonth(budget.value),
      pot: budgetPot.value,
      sessionKey: budget.value.sessionKey
    }
  }
  const state = tokenBudget.value
  if (!state) return null
  return {
    limit: state.monthlyLimit,
    remaining: budgetApi.tokenRemainingThisMonth(state),
    pot: state.amount,
    sessionKey: state.sessionKey
  }
})

const decimals = computed(() => (activeCurrency.value === 'sol' ? 9 : 6))
const unitLabel = computed(() => currencySymbol(activeCurrency.value))

function amountLabel(value: number): string {
  return `${formatTokenAmount(value, decimals.value)} ${unitLabel.value}`
}

/** What one unit of the selected currency is in base units. */
function toBaseUnits(value: number): number {
  return activeCurrency.value === 'sol'
    ? solToLamports(value)
    : Math.round(value * 1_000_000)
}

function shorten(value: string) {
  return `${value.slice(0, 4)}…${value.slice(-4)}`
}

async function refresh() {
  budget.value = null
  budgetPot.value = 0
  tokenBudgets.value = {}
  if (!publicKey.value) return
  try {
    const loaded = await budgetApi.loadBudget(publicKey.value)
    budget.value = loaded
    if (loaded) budgetPot.value = await budgetApi.availablePot(loaded)

    for (const currency of currencies) {
      if (currency === 'sol') continue
      const mint = mintForCurrency(currency, cluster)
      if (!mint) continue
      tokenBudgets.value[currency] = await budgetApi.loadTokenBudget(publicKey.value, mint)
    }
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
 * The one signature each currency's allowance ever asks for: funds the pot to
 * the limit, tops up the session key's fee float, and registers (or rotates)
 * the session key that will pay silently afterwards.
 */
async function saveLimit() {
  const buyer = publicKey.value
  if (!buyer) return

  failure.value = ''
  failureTitle.value = 'Something went wrong'
  success.value = ''

  const units = Number(limitAmount.value)
  const limit = toBaseUnits(units)
  if (!Number.isFinite(limit) || limit <= 0) {
    failure.value = `Enter a monthly limit greater than 0 ${unitLabel.value}.`
    return
  }

  busy.value = true
  try {
    const session = await ensureSessionSigner(buyer)
    const mint = activeMint.value
    const existing = activeCurrency.value === 'sol' ? budget.value : tokenBudget.value

    // 1. Would this wallet be able to fund the signature at all? A wallet that
    //    cannot cover it reports a useless "cannot simulate" warning and a bare
    //    "Internal error" after approval — so say what is missing first.
    if (!mint) {
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
    } else {
      // A token allowance also needs the tokens themselves, and a little SOL
      // for the account rent and fees the payout of the vault depends on.
      const cost = await budgetApi.openTokenBudgetCost(buyer, mint, session.address)
      const balance = Number(
        (await client.rpc.getBalance(buyer, { commitment: 'confirmed' }).send()).value
      )
      if (balance < cost.total) {
        failureTitle.value = 'Not enough SOL for fees'
        failure.value = `This wallet needs about ${lamportsToSol(cost.total)} SOL for the account rent and `
          + `network fee of a ${unitLabel.value} allowance. On devnet, https://faucet.solana.com sends free test SOL.`
        return
      }

      const held = await client.rpc
        .getTokenAccountBalance(await deriveAssociatedTokenAddress(buyer, mint), {
          commitment: 'confirmed'
        })
        .send()
        .then(balance => Number(balance.value.amount))
        .catch(() => 0)
      if (held < limit) {
        failureTitle.value = `Not enough ${unitLabel.value}`
        failure.value = `This wallet holds ${amountLabel(held)}, but the limit is ${amountLabel(limit)}. `
          + 'Lower the limit, or get free test tokens from https://faucet.circle.com and try again.'
        return
      }
    }

    // 2. Build it with a memo so the wallet and explorers can describe it,
    //    then simulate it before the popup: a transaction that reaches the
    //    wallet is one that will actually succeed.
    const instruction = mint
      ? buildInstruction(
          'open_token_budget',
          { monthly_limit: limit },
          {
            buyer,
            budget: await deriveTokenBudgetPda(buyer, mint),
            mint,
            buyer_token_account: await deriveAssociatedTokenAddress(buyer, mint),
            vault: await deriveAssociatedTokenAddress(
              await deriveTokenBudgetPda(buyer, mint),
              mint
            ),
            session_key: session.address
          }
        )
      : buildInstruction(
          'open_budget',
          { monthly_limit: limit },
          { buyer, budget: await deriveBudgetPda(buyer), session_key: session.address }
        )
    const memo = buildMemoInstruction(
      existing
        ? `Payvand: top up your own ${unitLabel.value} budget to ${amountLabel(limit)} (still your funds)`
        : `Payvand: keep ${amountLabel(limit)} as your monthly budget (still your funds, withdrawable anytime)`,
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
    success.value = existing
      ? 'Budget updated — your limit and pot are in sync again.'
      : `${unitLabel.value} allowance enabled. The tokens sit in an account derived from your wallet `
        + 'that only your private keys control — payments inside the limit need no approval, and '
        + '"Close budget" returns everything left at any time.'
    await refresh()
  } catch (error) {
    recordFailure(error)
  } finally {
    busy.value = false
  }
}

/** Reclaim everything left in one currency's allowance (1 signature). */
async function closeBudget() {
  const buyer = publicKey.value
  if (!buyer) return

  failure.value = ''
  failureTitle.value = 'Something went wrong'
  success.value = ''
  if (!view.value) {
    failure.value = 'There is no budget to close yet.'
    return
  }
  busy.value = true
  try {
    const mint = activeMint.value
    const instruction = mint
      ? buildInstruction(
          'close_token_budget',
          {},
          {
            buyer,
            budget: await deriveTokenBudgetPda(buyer, mint),
            mint,
            vault: await deriveAssociatedTokenAddress(
              await deriveTokenBudgetPda(buyer, mint),
              mint
            ),
            buyer_token_account: await deriveAssociatedTokenAddress(buyer, mint)
          }
        )
      : buildInstruction('close_budget', {}, { budget: await deriveBudgetPda(buyer), buyer })
    const memo = buildMemoInstruction(
      `Payvand: close ${unitLabel.value} budget and reclaim all funds`,
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
    await refresh()

    // The session key is only forgotten once no allowance is left to spend
    // with it — the other currencies' budgets still need it.
    const anyLeft
      = Boolean(budget.value) || Object.values(tokenBudgets.value).some(entry => Boolean(entry))
    if (!anyLeft) forgetSessionSigner(buyer)
    success.value = `${unitLabel.value} budget closed — everything left went back to your wallet.`
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
    description="Set your limit once a month — your wallet signs only that one time. The limit moves into an account derived from your own wallet, so only your private keys control it: payments inside it go through with no approval popups, and you can withdraw the whole amount back to your wallet at any moment."
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
        <!-- One allowance per currency: the card below follows the choice. -->
        <div
          v-if="currencies.length > 1"
          class="flex flex-wrap gap-2"
        >
          <UButton
            v-for="choice in currencies"
            :key="choice"
            :label="currencySymbol(choice)"
            size="sm"
            :color="activeCurrency === choice ? 'primary' : 'neutral'"
            :variant="activeCurrency === choice ? 'solid' : 'outline'"
            @click="activeCurrency = choice"
          />
        </div>

        <template v-if="connected && view">
          <div class="grid grid-cols-2 gap-4 sm:grid-cols-4">
            <div>
              <p class="text-sm text-muted">
                Monthly limit
              </p>
              <p class="text-lg font-semibold">
                {{ amountLabel(view.limit) }}
              </p>
            </div>
            <div>
              <p class="text-sm text-muted">
                Left this month
              </p>
              <p class="text-lg font-semibold text-success">
                {{ amountLabel(view.remaining) }}
              </p>
            </div>
            <div>
              <p class="text-sm text-muted">
                Pot (leftover included)
              </p>
              <p class="text-lg font-semibold">
                {{ amountLabel(view.pot) }}
              </p>
            </div>
            <div>
              <p class="text-sm text-muted">
                Session key
              </p>
              <p class="font-mono text-sm">
                {{ shorten(view.sessionKey) }}
              </p>
            </div>
          </div>

          <p class="text-sm text-muted">
            Your pot is held in a program-derived account — derived from, and spendable only
            with, your wallet's private key. Nothing is sent to anyone: the limit resets on the
            1st of each month, the pot keeps whatever you did not spend, and closing the budget
            returns every unit back to your wallet.
          </p>

          <UFormField
            :label="`Monthly limit (${unitLabel})`"
            help="Applied on the next signature — top up, lower, or raise it any time."
          >
            <UInput
              v-model="limitAmount"
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
              :label="`Close ${unitLabel} budget & reclaim funds`"
              color="error"
              variant="soft"
              icon="i-lucide-trash-2"
              :loading="busy || txLoading"
              @click="closeBudget"
            />
          </div>
        </template>

        <template v-else-if="connected">
          <UFormField :label="`Monthly limit (${unitLabel})`">
            <UInput
              v-model="limitAmount"
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
            This is the only wallet approval the feature needs. Your limit is not sent anywhere:
            it moves into an account derived from your wallet that only your private keys control,
            and the session key that signs future payments stays on this device. Payments inside
            the limit then go through with no popups, and you can withdraw the whole pot back at
            any moment.
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
