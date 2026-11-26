import type { AdminCode, AdminProduct, AdminShop, RecordPage } from './useAdminApi'

/** The seller-owned tables the records tab can browse. */
export const RECORD_KINDS = ['shops', 'products', 'codes'] as const
export type RecordKind = (typeof RECORD_KINDS)[number]

/** The API's own default page for record tables. */
export const RECORD_PAGE_SIZE = 25

/** A row the edit dialog can correct: shops and products; codes are delete-only. */
export type RecordEdit
  = | { kind: 'shop', row: AdminShop }
    | { kind: 'product', row: AdminProduct }

/**
 * Shops, products and discount codes, one table at a time, the way a database
 * console would show them. Each table keeps its own search and page, so
 * switching between them and back lands where the admin left off; a slow answer
 * for a table the admin already left is dropped rather than shown.
 */
export function useAdminRecords() {
  const { t } = useI18n()
  const api = useAdminApi()

  const recordKind = shallowRef<RecordKind>('shops')
  const shops = shallowRef<RecordPage<AdminShop> | null>(null)
  const products = shallowRef<RecordPage<AdminProduct> | null>(null)
  const codes = shallowRef<RecordPage<AdminCode> | null>(null)
  const searches = reactive<Record<RecordKind, string>>({ shops: '', products: '', codes: '' })
  const offsets = reactive<Record<RecordKind, number>>({ shops: 0, products: 0, codes: 0 })
  const recordsLoading = shallowRef(false)
  const recordsFailure = shallowRef('')
  const recordsBusy = shallowRef(false)

  /** The page on screen for a table, whichever table that is. */
  function pageOf(kind: RecordKind): RecordPage<unknown> | null {
    return kind === 'shops' ? shops.value : kind === 'products' ? products.value : codes.value
  }

  let latest = 0
  async function loadRecords() {
    const kind = recordKind.value
    const ticket = ++latest
    recordsFailure.value = ''
    recordsLoading.value = true
    try {
      const search = searches[kind].trim()
      const offset = offsets[kind]
      if (kind === 'shops') {
        const page = await api.fetchShops(search, RECORD_PAGE_SIZE, offset)
        if (ticket === latest) shops.value = page
      } else if (kind === 'products') {
        const page = await api.fetchProducts(search, RECORD_PAGE_SIZE, offset)
        if (ticket === latest) products.value = page
      } else {
        const page = await api.fetchCodes(search, RECORD_PAGE_SIZE, offset)
        if (ticket === latest) codes.value = page
      }
    } catch (error) {
      if (ticket === latest) recordsFailure.value = apiErrorMessage(error, t('admin.records.loadFailure'))
    } finally {
      if (ticket === latest) recordsLoading.value = false
    }
  }

  /** Switches table; a table seen before shows its last page at once and refreshes. */
  function showRecords(kind: RecordKind) {
    recordKind.value = kind
    void loadRecords()
  }

  function searchRecords(query: string) {
    searches[recordKind.value] = query
    offsets[recordKind.value] = 0
    void loadRecords()
  }

  function recordsPrevious() {
    const kind = recordKind.value
    offsets[kind] = Math.max(0, offsets[kind] - RECORD_PAGE_SIZE)
    void loadRecords()
  }

  function recordsNext() {
    offsets[recordKind.value] += RECORD_PAGE_SIZE
    void loadRecords()
  }

  /** Deletes one row after the admin confirms, stepping back off an emptied page. */
  async function removeRecord(kind: RecordKind, id: string, label: string) {
    const confirmed = window.confirm(t(`admin.records.confirm.${kind}`, { name: label }))
    if (!confirmed) return

    recordsBusy.value = true
    try {
      if (kind === 'shops') await api.deleteShop(id)
      else if (kind === 'products') await api.deleteProduct(id)
      else await api.deleteCode(id)
      const page = pageOf(kind)
      if (page && page.items.length === 1 && offsets[kind] > 0) {
        offsets[kind] = Math.max(0, offsets[kind] - RECORD_PAGE_SIZE)
      }
      await loadRecords()
    } catch (error) {
      recordsFailure.value = apiErrorMessage(error, t('admin.records.deleteFailure'))
    } finally {
      recordsBusy.value = false
    }
  }

  /**
   * Saves a correction. Answers with the API's complaint (a name too long, a
   * zero price) so the dialog can show it beside the fields, or `''` on success.
   */
  async function saveRecord(edit: RecordEdit, body: Record<string, unknown>): Promise<string> {
    recordsBusy.value = true
    try {
      if (edit.kind === 'shop') {
        await api.updateShop(edit.row.id, body as { name: string, description?: string })
      } else {
        await api.updateProduct(
          edit.row.id,
          body as { name: string, details?: string, category?: string, price_lamports: number }
        )
      }
      await loadRecords()
      return ''
    } catch (error) {
      return apiErrorMessage(error, t('admin.records.saveFailure'))
    } finally {
      recordsBusy.value = false
    }
  }

  return {
    recordKind,
    shops,
    products,
    codes,
    recordSearches: searches,
    recordOffsets: offsets,
    recordsPageSize: RECORD_PAGE_SIZE,
    recordsLoading,
    recordsFailure,
    recordsBusy,
    loadRecords,
    showRecords,
    searchRecords,
    recordsPrevious,
    recordsNext,
    removeRecord,
    saveRecord
  }
}
