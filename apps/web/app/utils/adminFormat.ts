/** Dates in the admin tables use the reader's own locale, as the old page did. */
export function adminDate(seconds: number): string {
  return new Date(seconds * 1000).toLocaleDateString()
}

export function adminDateTime(seconds: number): string {
  return new Date(seconds * 1000).toLocaleString()
}
