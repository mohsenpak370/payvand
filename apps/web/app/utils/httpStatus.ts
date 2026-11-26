/** ofetch carries HTTP failures in more than one place. */
export function httpStatus(error: unknown): number | undefined {
  if (typeof error !== 'object' || error === null) return undefined
  const candidate = error as {
    status?: number
    statusCode?: number
    response?: { status?: number }
  }
  return candidate.status ?? candidate.statusCode ?? candidate.response?.status
}
