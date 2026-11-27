import type { Address } from '@vue-solana/nuxt/kit'
import type { ComputedRef, Ref } from 'vue'
import { deriveBudgetPda, deriveTokenBudgetPda, type TokenBudgetState } from './budgetMath'
import {
  buildInstruction,
  buildMemoInstruction,
  compileWireTransaction,
  deriveAssociatedTokenAddress,
  deriveConfigPda
} from './usePaymentProgram'
import { ensureSessionSigner } from './useSessionKey'
import type { BudgetAllowanceView } from './useBudgetAllowances'
import type { useBudgetMessages } from './useBudgetMessages'
import type { BudgetPageState } from './useBudgetPageState'

/**
 * Sets, raises or lowers the allowance on screen.
 *
 * Each currency's allowance asks for exactly one wallet approval in its life:
 * funding the pot to the limit, topping up the session key's fee float, and
 * registering (or rotating) the session key that pays silently afterwards.
 * Everything a person can change about the allowance is that same approval, and
 * a *different* limit set mid-month starts the month over, so the limit they
 * typed is the limit they can actually spend (see `applyLimit`).
 */
export function useBudgetApply(deps: {
  state: BudgetPageState
  messages: ReturnType<typeof useBudgetMessages>
  budgetApi: ReturnType<typeof useBudget>
  client: ReturnType<typeof useSolanaClient>['client']
  execute: ReturnType<typeof useSolanaSignAndSendTransaction>['execute']
  publicKey: Ref<Address | null>
  activeMint: ComputedRef<Address | null>
  tokenBudget: ComputedRef<TokenBudgetState | null>
  view: ComputedRef<BudgetAllowanceView | null>
  refresh: () => Promise<void>
  amountLabel: (amount: number) => string
  toBaseUnits: (amount: number) => number
  unitLabel: ComputedRef<string>
}) {
  const { state, messages, budgetApi, client, execute, publicKey } = deps
  const { activeMint, tokenBudget, view, refresh, amountLabel, toBaseUnits, unitLabel } = deps
  const { busy, failure, failureTitle, success, activeCurrency, limitAmount, budget } = state
  const { recordFailure, resetMessages } = messages
  const { t } = useI18n()

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

  /**
   * Puts the pot back to the monthly limit mid-month, in one approval. What was
   * already spent this month still counts: a refill is not a way past the
   * buyer's own ceiling, only a new limit starts the month over.
   */
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

    resetMessages()

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
      //
      //    A different limit set mid-month is a new month. The program keeps
      //    `spent` across a top-up on purpose — that is what stops a refill from
      //    being a way past your own ceiling — so setting 20 after spending 5 of
      //    10 would leave 15 spendable, not 20. Retiring the allowance and
      //    opening it again in this same transaction is the one start-over the
      //    deployed program allows: one approval, the same session key, and the
      //    counter back at zero. Only a saved limit that differs from the
      //    current one does this: a refill at the same limit keeps the spent
      //    part counting, and a rotation moves the same allowance to this
      //    device, so both stay a plain top-up.
      const current = view.value
      const spentThisMonth
        = current && !current.rolledOver ? Math.max(0, current.limit - current.remaining) : 0
      const restarts = intent === 'save'
        && Boolean(existing)
        && current !== null
        && limit !== current.limit
        && spentThisMonth > 0

      const budgetAddress = mint
        ? await deriveTokenBudgetPda(buyer, mint)
        : await deriveBudgetPda(buyer)
      const reopen = mint
        ? buildInstruction(
            'open_token_budget',
            { monthly_limit: limit },
            {
              buyer,
              budget: budgetAddress,
              mint,
              buyer_token_account: await deriveAssociatedTokenAddress(buyer, mint),
              vault: await deriveAssociatedTokenAddress(budgetAddress, mint),
              session_key: session.address
            }
          )
        : buildInstruction(
            'open_budget',
            { monthly_limit: limit },
            {
              buyer,
              budget: budgetAddress,
              session_key: session.address,
              config: await deriveConfigPda()
            }
          )
      // Retiring first returns the pot (and the rent) before the reopen asks for
      // the new limit, so the wallet is only ever asked for the difference.
      const retire = !restarts
        ? null
        : mint
          ? buildInstruction(
              'close_token_budget',
              {},
              {
                buyer,
                budget: budgetAddress,
                mint,
                buyer_token_account: await deriveAssociatedTokenAddress(buyer, mint),
                vault: await deriveAssociatedTokenAddress(budgetAddress, mint)
              }
            )
          : buildInstruction('close_budget', {}, { budget: budgetAddress, buyer })

      const freshMonth = restarts ? ' for a new month' : ''
      let memoText = `Payvand: keep ${amountLabel(limit)} as your monthly budget `
        + '(still your funds, withdrawable anytime)'
      if (intent === 'rotate') {
        memoText = 'Payvand: pay from this device, same limit, other devices revoked'
      } else if (intent === 'refill') {
        memoText = `Payvand: refill your own ${unitLabel.value} budget to ${amountLabel(limit)} `
          + '(still your funds)'
      } else if (existing) {
        memoText = `Payvand: top up your own ${unitLabel.value} budget to ${amountLabel(limit)}`
          + `${freshMonth} (still your funds)`
      }
      const memo = buildMemoInstruction(memoText, buyer)
      const wire = await compileWireTransaction({
        instruction: retire ? [retire, reopen, memo] : [reopen, memo],
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
        success.value = restarts
          ? t('budget.success.restarted', { limit: amountLabel(limit) })
          : t('budget.success.updated')
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

  return { saveLimit, rotateToThisDevice, refill }
}
