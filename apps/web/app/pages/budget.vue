<script setup lang="ts">
import { address, type Address } from '@vue-solana/nuxt/kit'
import type { BudgetState, TokenBudgetState } from '~/composables/useBudget'
import type { SupportedCurrency } from '~/composables/usePaymentApi'
import { currencySymbol, formatTokenAmount } from '~/composables/usePaymentApi'
import { currenciesForCluster, mintForCurrency } from '~/composables/usePaymentProgram'

const { t } = useI18n()

useSeoMeta({ title: t('budget.metaTitle') })

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
/**
 * Low on purpose: the session key pays without the wallet, so the limit is
 * also the most a device holding it can ever spend on its own.
 */
const DEFAULT_LIMITS: Record<SupportedCurrency, string> = { sol: '0.1', usdc: '1', eurc: '1' }
const limitAmount = ref(DEFAULT_LIMITS.sol)
const tokenBudgets = ref<Partial<Record<SupportedCurrency, TokenBudgetState | null>>>({})

const busy = ref(false)
const failure = ref('')
const failureTitle = ref(t('budget.errors.genericTitle'))
const success = ref('')
const budget = ref<BudgetState | null>(null)
const budgetPot = ref(0)
/** The session key this device holds, if any — the only key that can pay here. */
const deviceKeyAddress = ref<Address | null>(null)
/** What this device's key still holds that no allowance can spend. */
const recoverableFloat = ref(0)
const sweeping = ref(false)
/**
 * Whether the last read of the budgets succeeded. A failed read leaves them
 * null, which must not be mistaken for "no allowance needs this key": that
 * guess would sweep a live key's float and forget it.
 */
const budgetsRead = ref(false)

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

/**
 * True when the allowance on screen is signed by a key this device does not
 * hold: another installation opened it, or this browser was never set up.
 */
const foreignDevice = computed(
  () => Boolean(view.value && view.value.sessionKey !== deviceKeyAddress.value)
)

/**
 * Every session key an allowance on this page names. A key of ours that is not
 * in here no longer pays anything, so its float is ours to take back.
 */
const liveSessionKeys = computed(() =>
  [budget.value?.sessionKey, ...Object.values(tokenBudgets.value).map(entry => entry?.sessionKey)]
    .filter((key): key is Address => Boolean(key))
)

/** What it would take to put the pot back to the full monthly limit. */
const refillNeeded = computed(() => {
  const current = view.value
  return current ? Math.max(0, current.limit - current.pot) : 0
})

/**
 * The SOL cost of the signature that would apply this limit, row by row. The
 * session-key float comes from `session_fee_float` on chain (the token flows
 * still read the program's constant), so a policy change moves this too.
 */
const openCost = ref<{ rows: { key: string, lamports: number }[], total: number } | null>(null)
const openCostLoading = ref(false)

/** The limit being priced, in base units; 0 while the field holds no amount. */
const limitBaseUnits = computed(() => {
  const units = toBaseUnits(Number(limitAmount.value))
  return Number.isFinite(units) && units > 0 ? units : 0
})

/** A real limit the chain could not price — as opposed to one still on its way. */
const openCostMissing = computed(
  () => limitBaseUnits.value > 0 && !openCostLoading.value && !openCost.value
)

async function refreshOpenCost() {
  const buyer = publicKey.value
  const limit = limitBaseUnits.value
  if (!buyer || limit <= 0) {
    openCost.value = null
    openCostLoading.value = false
    return
  }
  openCostLoading.value = true
  try {
    const sessionAddress = deviceKeyAddress.value ? address(deviceKeyAddress.value) : undefined
    const mint = activeMint.value
    if (mint) {
      const cost = await budgetApi.openTokenBudgetCost(buyer, mint, sessionAddress)
      openCost.value = {
        rows: [
          { key: 'feeFloat', lamports: cost.feeFloat },
          { key: 'rent', lamports: cost.rent },
          { key: 'fee', lamports: cost.fee }
        ],
        total: cost.total
      }
    } else {
      const cost = await budgetApi.openBudgetCost(buyer, limit, sessionAddress)
      openCost.value = {
        rows: [
          { key: 'topUp', lamports: cost.topUp },
          { key: 'feeFloat', lamports: cost.feeFloat },
          { key: 'rent', lamports: cost.rent },
          { key: 'fee', lamports: cost.fee }
        ],
        total: cost.total
      }
    }
  } catch {
    // A quote that needs the chain and cannot reach it says so, rather than
    // promising a number.
    openCost.value = null
  } finally {
    openCostLoading.value = false
  }
}

// Every keystroke in the limit field would be a handful of RPC reads, so the
// quote waits for the typing to pause.
let costTimer: ReturnType<typeof setTimeout> | undefined

function scheduleOpenCost() {
  clearTimeout(costTimer)
  costTimer = setTimeout(() => {
    void refreshOpenCost()
  }, 300)
}

watch([publicKey, activeMint], () => {
  void refreshOpenCost()
})
// Set by `refresh()` once it knows which key this device holds.
watch(deviceKeyAddress, () => {
  void refreshOpenCost()
})
// The top-up is `limit - pot`, so a refill, a save or a close moves it: the
// quote follows the allowance state rather than only the field.
watch(view, () => {
  void refreshOpenCost()
})
watch(limitAmount, scheduleOpenCost)
onBeforeUnmount(() => {
  clearTimeout(costTimer)
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

/**
 * A key this device holds that no remaining allowance names is one it was
 * rotated out of, or one whose budget is gone: nothing here can spend its
 * float, so it becomes a visible recover action instead of lost lamports.
 */
async function refreshRecoverableFloat() {
  const key = deviceKeyAddress.value
  if (!key || !budgetsRead.value || liveSessionKeys.value.includes(key)) {
    recoverableFloat.value = 0
    return
  }
  recoverableFloat.value = await budgetApi.sweepableFloat(key).catch(() => 0)
}

async function refresh() {
  budget.value = null
  budgetPot.value = 0
  tokenBudgets.value = {}
  budgetsRead.value = false
  if (!publicKey.value) {
    deviceKeyAddress.value = null
    recoverableFloat.value = 0
    return
  }
  deviceKeyAddress.value = (await peekSessionSigner(publicKey.value))?.address ?? null
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
    budgetsRead.value = true
  } catch (error) {
    failure.value = error instanceof Error ? error.message : t('budget.errors.readFailure')
  }
  await refreshRecoverableFloat()
}

watch(publicKey, refresh, { immediate: true })

// A fresh low default for the currency being looked at, unless it was edited.
watch(activeCurrency, (currency, previous) => {
  if (limitAmount.value === DEFAULT_LIMITS[previous]) {
    limitAmount.value = DEFAULT_LIMITS[currency]
  }
})

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
      return t('budget.errors.rejected')
    case 'RPC_FAILURE':
      return t('budget.errors.rpcFailure')
    case 'TRANSACTION_TIMEOUT':
      return t('budget.errors.timeout')
    default:
      return error instanceof Error && error.message ? error.message : t('budget.errors.rejectedFallback')
  }
}

function recordFailure(error: unknown) {
  failureTitle.value = solanaErrorCode(error) ?? t('budget.errors.notSentTitle')
  failure.value = describeFailure(error)
}

function resetMessages() {
  failure.value = ''
  failureTitle.value = t('budget.errors.genericTitle')
  success.value = ''
}

async function saveLimit() {
  const units = Number(limitAmount.value)
  const limit = toBaseUnits(units)
  if (!Number.isFinite(limit) || limit <= 0) {
    failure.value = t('budget.errors.badLimit', { currency: unitLabel.value })
    return
  }
  resetMessages()
  await applyLimit(limit, 'save')
}

/**
 * Moves this currency's allowance onto this device: the same instruction the
 * first setup uses, which rewrites `session_key` — so every other device stops
 * being able to spend. The limit written is what the pot already holds, since
 * `open_*_budget` tops the pot up to the limit it is given; rotating at the old
 * limit would pull the spent part back out of the wallet.
 *
 * The key it installs is this device's own, not a fresh one. The key being
 * replaced belongs to the other device and could only be swept from there,
 * while a fresh key here would strand the float this device already holds and
 * buy a second one — reusing it puts that float back to work instead.
 */
async function rotateToThisDevice() {
  const current = view.value
  if (!current) return
  resetMessages()
  const funded = Math.min(current.limit, current.pot)
  if (funded <= 0) {
    failureTitle.value = t('budget.errors.nothingToTakeOver')
    failure.value = t('budget.errors.emptyPot', { currency: unitLabel.value })
    return
  }
  await applyLimit(funded, 'rotate')
}

/** Puts the pot back to the monthly limit mid-month, in one approval. */
async function refill() {
  const current = view.value
  if (!current) return
  resetMessages()
  await applyLimit(current.limit, 'refill')
}

/**
 * The one signature each currency's allowance ever asks for: funds the pot to
 * the limit, tops up the session key's fee float, and registers (or rotates)
 * the session key that will pay silently afterwards.
 */
async function applyLimit(limit: number, intent: 'save' | 'rotate' | 'refill') {
  const buyer = publicKey.value
  if (!buyer) return

  failure.value = ''
  failureTitle.value = t('budget.errors.genericTitle')
  success.value = ''

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
        failureTitle.value = t('budget.errors.notEnoughSol')
        failure.value = t('budget.errors.notEnoughSolBody', {
          balance: lamportsToSol(balance),
          limit: lamportsToSol(limit),
          total: lamportsToSol(cost.total),
          topUp: lamportsToSol(cost.topUp),
          feeFloat: lamportsToSol(cost.feeFloat),
          rent: lamportsToSol(cost.rent),
          fee: lamportsToSol(cost.fee)
        })
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
        failureTitle.value = t('budget.errors.notEnoughSolFees')
        failure.value = t('budget.errors.feeBody', {
          total: lamportsToSol(cost.total),
          currency: unitLabel.value
        })
        return
      }

      // Only the gap between the limit and what the vault already holds comes
      // out of the wallet; a takeover moves nothing at all.
      const held = await client.rpc
        .getTokenAccountBalance(await deriveAssociatedTokenAddress(buyer, mint), {
          commitment: 'confirmed'
        })
        .send()
        .then(balance => Number(balance.value.amount))
        .catch(() => 0)
      const needed = Math.max(0, limit - (view.value?.pot ?? 0))
      if (intent !== 'rotate' && held < needed) {
        failureTitle.value = t('budget.errors.notEnoughToken', { currency: unitLabel.value })
        failure.value = t('budget.errors.tokenBody', {
          held: amountLabel(held),
          needed: amountLabel(needed),
          limit: amountLabel(limit)
        })
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
          {
            buyer,
            budget: await deriveBudgetPda(buyer),
            session_key: session.address,
            config: await deriveConfigPda()
          }
        )
    let memoText = `Payvand: keep ${amountLabel(limit)} as your monthly budget `
      + '(still your funds, withdrawable anytime)'
    if (intent === 'rotate') {
      memoText = 'Payvand: pay from this device, same limit, other devices revoked'
    } else if (intent === 'refill') {
      memoText = `Payvand: refill your own ${unitLabel.value} budget to ${amountLabel(limit)} `
        + '(still your funds)'
    } else if (existing) {
      memoText = `Payvand: top up your own ${unitLabel.value} budget to ${amountLabel(limit)} `
        + '(still your funds)'
    }
    const memo = buildMemoInstruction(memoText, buyer)
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
    if (intent === 'rotate') {
      success.value = t('budget.success.rotated', { limit: amountLabel(limit) })
    } else if (intent === 'refill') {
      success.value = t('budget.success.refilled', { limit: amountLabel(limit) })
    } else if (existing) {
      success.value = t('budget.success.updated')
    } else {
      success.value = t('budget.success.enabled', { currency: unitLabel.value })
    }
    await refresh()
  } catch (error) {
    recordFailure(error)
  } finally {
    busy.value = false
  }
}

/**
 * Sweeps this device's float home and only then forgets the key — the one place
 * a session key is dropped, so no path can forget one still holding lamports.
 * Throws with the key and its float intact, which is what leaves a retry.
 */
async function sweepHomeAndForget(buyer: Address): Promise<number> {
  const { swept } = await budgetApi.sweepSessionFloat(buyer)
  forgetSessionSigner(buyer)
  deviceKeyAddress.value = null
  recoverableFloat.value = 0
  return swept
}

/**
 * Sends a stranded float home after the key stopped paying for anything: a
 * rotated-out device, or a budget closed from another one.
 */
async function recoverFloat() {
  const buyer = publicKey.value
  const key = deviceKeyAddress.value
  if (!buyer || !key || !budgetsRead.value || liveSessionKeys.value.includes(key)) return
  resetMessages()
  sweeping.value = true
  try {
    const swept = await sweepHomeAndForget(buyer)
    success.value = t('budget.success.floatRecovered', { amount: lamportsToSol(swept) })
  } catch (error) {
    recordFailure(error)
  } finally {
    sweeping.value = false
  }
}

/**
 * Hands the float back after the last allowance is gone, and says what happened
 * to it: the close worked either way, so the message must not claim everything
 * came back when the float is still on this device's key.
 */
async function handBackFloat(buyer: Address) {
  try {
    await sweepHomeAndForget(buyer)
    success.value = t('budget.success.closed', { currency: unitLabel.value })
  } catch (error) {
    failureTitle.value = t('budget.errors.floatNotRecovered')
    await refreshRecoverableFloat()
    failure.value = t('budget.errors.floatUnsweptBody', {
      reason: describeFailure(error),
      amount: lamportsToSol(recoverableFloat.value)
    })
  }
}

/**
 * Reclaim one currency's pot and its account rent (1 signature). The session
 * key's float follows only when this was the last allowance — the other
 * currencies' budgets still pay with it.
 */
async function closeBudget() {
  const buyer = publicKey.value
  if (!buyer) return

  failure.value = ''
  failureTitle.value = t('budget.errors.genericTitle')
  success.value = ''
  if (!view.value) {
    failure.value = t('budget.errors.noBudgetToClose')
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
    // with it — the other currencies' budgets still need it — and only after
    // its float is back, which `handBackFloat` does and reports. An unreadable
    // budget list answers neither question, so the key and its float stay put.
    if (!budgetsRead.value) {
      failure.value = t('budget.errors.readFailure')
      return
    }
    const anyLeft
      = Boolean(budget.value) || Object.values(tokenBudgets.value).some(entry => Boolean(entry))
    if (anyLeft) {
      success.value = t('budget.success.closed', { currency: unitLabel.value })
    } else {
      await handBackFloat(buyer)
    }
  } catch (error) {
    recordFailure(error)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <UPageSection
    :title="$t('budget.title')"
    :description="$t('budget.description')"
  >
    <UCard>
      <template #header>
        <div class="flex items-center justify-between gap-3">
          <h2 class="text-base font-semibold">
            {{ $t('budget.cardTitle') }}
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
                {{ $t('budget.display.monthlyLimit') }}
              </p>
              <p class="text-lg font-semibold">
                {{ amountLabel(view.limit) }}
              </p>
            </div>
            <div>
              <p class="text-sm text-muted">
                {{ $t('budget.display.leftThisMonth') }}
              </p>
              <p class="text-lg font-semibold text-success">
                {{ amountLabel(view.remaining) }}
              </p>
            </div>
            <div>
              <p class="text-sm text-muted">
                {{ $t('budget.display.pot') }}
              </p>
              <p class="text-lg font-semibold">
                {{ amountLabel(view.pot) }}
              </p>
            </div>
            <div>
              <p class="text-sm text-muted">
                {{ $t('budget.display.sessionKey') }}
              </p>
              <p class="font-mono text-sm">
                {{ shorten(view.sessionKey) }}
              </p>
              <p
                v-if="foreignDevice"
                class="text-xs text-muted"
              >
                {{ $t('budget.display.anotherDevice') }}
              </p>
            </div>
          </div>

          <p class="text-sm text-muted">
            <span class="font-medium text-highlighted">{{ $t('budget.whereMoney.title') }}</span>
            {{ $t('budget.whereMoney.body') }}
          </p>

          <p class="text-sm text-muted">
            <span class="font-medium text-highlighted">{{ $t('budget.keepSmall.title') }}</span>
            {{ $t('budget.keepSmall.body') }}
            <NuxtLink
              to="/learn/two-wallets"
              class="font-medium text-primary underline-offset-4 hover:underline"
            >
              {{ $t('budget.keepSmall.twoWallets') }}
            </NuxtLink>.
          </p>

          <UAlert
            v-if="foreignDevice"
            color="info"
            variant="subtle"
            icon="i-lucide-smartphone"
            :title="$t('budget.foreign.title')"
            :description="deviceKeyAddress
              ? $t('budget.foreign.withKey', { key: shorten(deviceKeyAddress) })
              : $t('budget.foreign.description')"
          >
            <template #actions>
              <UButton
                :label="$t('budget.foreign.action')"
                size="sm"
                :loading="busy || txLoading"
                @click="rotateToThisDevice"
              />
            </template>
          </UAlert>

          <UFormField
            :label="$t('budget.limitLabel', { currency: unitLabel })"
            :help="$t('budget.limitHelp')"
          >
            <UInput
              v-model="limitAmount"
              type="number"
              min="0"
              step="0.1"
              class="w-full"
              :placeholder="DEFAULT_LIMITS[activeCurrency]"
            />
          </UFormField>

          <BudgetOpenCost :cost="openCost" />
          <p
            v-if="openCostMissing"
            class="text-xs text-muted"
          >
            {{ $t('budget.cost.unavailable') }}
          </p>

          <UAlert
            color="warning"
            variant="subtle"
            icon="i-lucide-shield-alert"
            :title="$t('budget.lowLimitTitle')"
            :description="$t('budget.lowLimitWarning')"
          />

          <div class="flex flex-wrap gap-2">
            <UButton
              v-if="refillNeeded > 0"
              :label="$t('budget.refill', { amount: amountLabel(view.limit) })"
              icon="i-lucide-piggy-bank"
              :loading="busy || txLoading"
              @click="refill"
            />
            <UButton
              :label="$t('budget.saveLimit')"
              icon="i-lucide-refresh-cw"
              :loading="busy || txLoading"
              @click="saveLimit"
            />
            <UButton
              :label="$t('budget.closeBudget', { currency: unitLabel })"
              color="error"
              variant="soft"
              icon="i-lucide-trash-2"
              :loading="busy || txLoading"
              @click="closeBudget"
            />
          </div>

          <p class="text-sm text-muted">
            <span class="font-medium text-highlighted">{{ $t('budget.killSwitch.title') }}</span>
            {{ $t('budget.killSwitch.body') }}
          </p>
        </template>

        <template v-else-if="connected">
          <UFormField :label="$t('budget.limitLabel', { currency: unitLabel })">
            <UInput
              v-model="limitAmount"
              type="number"
              min="0"
              step="0.1"
              class="w-full"
              :placeholder="DEFAULT_LIMITS[activeCurrency]"
            />
          </UFormField>

          <BudgetOpenCost :cost="openCost" />
          <p
            v-if="openCostMissing"
            class="text-xs text-muted"
          >
            {{ $t('budget.cost.unavailable') }}
          </p>

          <UAlert
            color="warning"
            variant="subtle"
            icon="i-lucide-shield-alert"
            :title="$t('budget.lowLimitTitle')"
            :description="$t('budget.lowLimitWarning')"
          />

          <UButton
            :label="$t('budget.enable')"
            icon="i-lucide-wallet"
            :loading="busy || txLoading"
            @click="saveLimit"
          />

          <p class="text-sm text-muted">
            <span class="font-medium text-highlighted">{{ $t('budget.oneApproval.title') }}</span>
            {{ $t('budget.oneApproval.body') }}
          </p>

          <p class="text-sm text-muted">
            {{ $t('budget.startSmall') }}
            <NuxtLink
              to="/learn/two-wallets"
              class="font-medium text-primary underline-offset-4 hover:underline"
            >
              {{ $t('budget.keepSmall.twoWallets') }}
            </NuxtLink>.
          </p>
        </template>

        <p
          v-else
          class="text-sm text-muted"
        >
          {{ $t('budget.connectPrompt') }}
        </p>

        <UAlert
          v-if="connected && budgetsRead && recoverableFloat > 0 && !busy"
          color="warning"
          variant="subtle"
          icon="i-lucide-hand-coins"
          :title="$t('budget.recover.title')"
          :description="$t('budget.recover.body', { amount: lamportsToSol(recoverableFloat) })"
        >
          <template #actions>
            <UButton
              :label="$t('budget.recover.action', { amount: lamportsToSol(recoverableFloat) })"
              size="sm"
              :loading="busy || sweeping || txLoading"
              @click="recoverFloat"
            />
          </template>
        </UAlert>

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
          :title="$t('common.done')"
          :description="success"
        />
      </div>
    </UCard>
  </UPageSection>
</template>
