import type { BudgetPageState } from './useBudgetPageState'

/**
 * Turns whatever went wrong into messages a person can act on, and keeps the
 * success line beside them.
 *
 * Every mutation on the page reports through here, so a wallet refusal reads
 * the same whether it came from a save, a takeover or a close.
 */
export function useBudgetMessages(state: BudgetPageState) {
  const { failure, failureTitle, success } = state
  const { t } = useI18n()

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
        return error instanceof Error && error.message
          ? error.message
          : t('budget.errors.rejectedFallback')
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

  return { describeFailure, recordFailure, resetMessages }
}
