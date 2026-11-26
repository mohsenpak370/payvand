import { address } from '@vue-solana/nuxt/kit'
import type {
  DiscountCode,
  PaymentItem,
  PaymentRecord,
  SupportedCurrency
} from '~/composables/usePaymentApi'
import { currencySymbol, formatTokenAmount } from '~/composables/usePaymentApi'
import { CARD_DESIGNS, cardDesignOf, type CardDesign } from '~/utils/cardDesigns'

const CODE_SIZE = 320

interface UseSellPaymentResultOptions {
  discounts: Ref<DiscountCode[]>
  priceCurrency: Ref<SupportedCurrency>
}

export function useSellPaymentResult(options: UseSellPaymentResultOptions) {
  const { cluster } = useRuntimeConfig().public
  const api = usePaymentApi()
  const { playSuccess } = useTransactionSounds()
  const { t } = useI18n()

  // The code is drawn at 320 and PaymentQr pads it by p-3 (0.75rem) a side, so
  // the piece of paper the seller holds up is 344 wide. The button that presents
  // it takes that width rather than the card's, so the two line up.
  const codePaperWidth = `${CODE_SIZE + 24}px`

  const record = ref<PaymentRecord | null>(null)
  const qrPayload = ref('')
  // The receipt this payment carries. Kept here rather than read back off the
  // record: a status refresh replaces the record with the chain's copy, which
  // knows the memo and the amount but nothing about what was sold.
  const createdItems = ref<PaymentItem[]>([])
  const codeOpen = ref(false)
  // What a seller holds up to the customer's phone: the code, the name, the
  // description and the price on their own screen, without the form around them.
  const presentOpen = ref(false)
  const priceLamports = ref(0)
  const paidCode = ref<string | null>(null)

  /**
   * The look of the page, chosen when the seller is about to open it. It is
   * the one part of a code that can still change after the invoice exists,
   * because it changes nothing about what is owed.
   */
  const design = ref<CardDesign>('classic')
  const designOpen = ref(false)
  const designOptions = computed(() => CARD_DESIGNS.map(value => ({
    value,
    label: t(`sell.designs.${value}`)
  })))

  const copiedCode = ref(false)
  const copiedLink = ref(false)
  const payloadBox = ref<HTMLElement>()
  let copiedTimer: ReturnType<typeof setTimeout> | undefined
  let linkTimer: ReturnType<typeof setTimeout> | undefined
  let timer: ReturnType<typeof setInterval> | undefined

  /** A price in the money the customer will hand over. */
  function priceLabel(option: { currency: SupportedCurrency, amount: number }): string {
    return option.currency === 'sol'
      ? `${lamportsToSol(option.amount)} SOL`
      : `${formatTokenAmount(option.amount, 6)} ${currencySymbol(option.currency)}`
  }

  /** What a carried discount code took off, in the currency this code charges. */
  const savedLabel = computed(() => {
    const option = record.value?.options?.[0]
    const currency = option?.currency ?? options.priceCurrency.value
    // The invoice's own price, in its own base units: a token invoice keeps its
    // amount in `options`, while `amount_lamports` is zero for it.
    const total = option?.amount ?? record.value?.amount_lamports ?? 0
    const saved = priceLamports.value - total
    return saved > 0 ? priceLabel({ currency, amount: saved }) : ''
  })

  /** The public page this invoice opens — the link to share with the buyer. */
  const shareLink = computed(() => (qrPayload.value ? paymentUrl(qrPayload.value) : ''))

  /**
   * Writes the chosen look into the code itself, so the QR, the copied link and
   * the page all say the same thing. Only `design` is replaced: the invoice's
   * own fields — address, price, memo — are what was agreed when it was made.
   */
  function withDesign(payload: string, chosen: CardDesign): string {
    try {
      return JSON.stringify({ ...JSON.parse(payload), design: chosen })
    } catch {
      return payload
    }
  }

  /** The page, in the design the seller just picked, in its own tab. */
  function openPaymentPage() {
    qrPayload.value = withDesign(qrPayload.value, design.value)
    const url = shareLink.value
    designOpen.value = false
    if (url) window.open(url, '_blank', 'noopener')
  }

  const statusColor = (status: string) =>
    status === 'settled' ? 'success' : status === 'missing' || status === 'pending' ? 'warning' : 'neutral'

  /** How to print whatever an invoice was settled for, in its own currency. */
  function paidAmountLabel(current: PaymentRecord): string {
    const option = current.options?.[0]
    if (option && option.currency !== 'sol') {
      return `${formatTokenAmount(option.amount, option.decimals)} ${currencySymbol(option.currency)}`
    }
    return `${lamportsToSol(current.amount_lamports)} SOL`
  }

  function applyCreated(created: {
    payment: PaymentRecord
    qrPayload: string
    priceLamports: number
    items: PaymentItem[]
  }) {
    record.value = created.payment
    qrPayload.value = created.qrPayload
    priceLamports.value = created.priceLamports
    paidCode.value = null
    createdItems.value = created.items
    // A fresh code wears the plain card until the seller picks one.
    design.value = cardDesignOf(undefined)
  }

  function reset() {
    record.value = null
    qrPayload.value = ''
    presentOpen.value = false
    paidCode.value = null
    priceLamports.value = 0
    createdItems.value = []
    design.value = cardDesignOf(undefined)
    designOpen.value = false
  }

  /**
   * Every address this invoice can be settled on: its currency at full price,
   * plus the same currency at each discount code's price. Whichever gets funded
   * says what was paid and which code the buyer used.
   */
  async function watchTargets(): Promise<{ pda: string, code: string | null }[]> {
    const current = record.value
    if (!current) return []

    const seller = address(current.seller)
    const reference = address(current.reference)
    const invoiceOptions = current.options?.length
      ? current.options
      : [{
          currency: 'sol' as SupportedCurrency,
          mint: '',
          decimals: 9,
          amount: current.amount_lamports,
          payment_pda: current.payment_pda
        }]

    const targets: { pda: string, code: string | null }[] = []
    for (const option of invoiceOptions) {
      targets.push({ pda: option.payment_pda, code: null })
      for (const discount of options.discounts.value) {
        const amount = discountedLamports(option.amount, discount.percent_off)
        if (amount <= 0 || amount === option.amount) continue
        const pda = option.currency === 'sol'
          ? await derivePaymentPda(seller, reference, amount)
          : await deriveTokenPaymentPda(seller, reference, address(option.mint), amount)
        targets.push({ pda: pda.toString(), code: discount.code })
      }
    }
    return targets
  }

  async function refreshStatus() {
    const current = record.value
    if (!current || current.status === 'settled') return
    try {
      const targets = await watchTargets()
      const results = await api.watchPayments(
        targets.map(target => target.pda),
        { cluster }
      )

      const settledIndex = results.findIndex(result => result.status === 'settled')
      const settled = settledIndex >= 0 ? results[settledIndex] : undefined
      if (settled) {
        // The chain knows the memo but not the name or description, so keep
        // this invoice's own copy once the settled record replaces it.
        record.value = {
          ...settled,
          name: settled.name || current.name,
          description: settled.description || current.description
        }
        paidCode.value = targets[settledIndex]?.code ?? null
        // A code the seller attached is spent by the buyer who paid it: record
        // the use now, while both the code and the payer are known.
        if (paidCode.value) {
          void api.recordDiscountUse({
            code: paidCode.value,
            seller: settled.seller || current.seller,
            invoice: settled.payment_pda
          }).catch(() => {
            // The API stores the settlement when it reads it; a later read retries.
          })
        }
        // The seller is holding this screen up for the customer, so the payment
        // arriving is worth hearing without having to watch the badge.
        playSuccess()
        return
      }

      // Nothing paid yet: keep the canonical details, refresh the badge.
      record.value = {
        ...current,
        status: results.some(result => result.status === 'pending') ? 'pending' : 'missing',
        settled_by: null,
        settled_at_unix: 0
      }
    } catch {
      // Keep the last known status; the next tick retries.
    }
  }

  /**
   * Copying is the whole point of showing the code, so a refused clipboard must
   * not fail silently: the payload is selected instead, leaving one keystroke.
   */
  async function copyPayload() {
    try {
      await navigator.clipboard.writeText(qrPayload.value)
    } catch {
      const code = payloadBox.value?.querySelector('code')
      if (code) {
        const range = document.createRange()
        range.selectNodeContents(code)
        const selection = window.getSelection()
        selection?.removeAllRanges()
        selection?.addRange(range)
      }
      return
    }

    copiedCode.value = true
    clearTimeout(copiedTimer)
    copiedTimer = setTimeout(() => {
      copiedCode.value = false
    }, 2_000)
  }

  async function copyLink() {
    try {
      await navigator.clipboard.writeText(shareLink.value)
    } catch {
      return
    }
    copiedLink.value = true
    clearTimeout(linkTimer)
    linkTimer = setTimeout(() => {
      copiedLink.value = false
    }, 2_000)
  }

  onMounted(() => {
    timer = setInterval(refreshStatus, 5_000)
  })

  onBeforeUnmount(() => {
    if (timer) clearInterval(timer)
    clearTimeout(copiedTimer)
    clearTimeout(linkTimer)
  })

  return {
    CODE_SIZE,
    codePaperWidth,
    record,
    qrPayload,
    createdItems,
    codeOpen,
    presentOpen,
    priceLamports,
    paidCode,
    design,
    designOptions,
    designOpen,
    openPaymentPage,
    copiedCode,
    copiedLink,
    payloadBox,
    savedLabel,
    shareLink,
    statusColor,
    paidAmountLabel,
    applyCreated,
    reset,
    refreshStatus,
    copyPayload,
    copyLink
  }
}
