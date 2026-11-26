import type {
  DiscountCode,
  PaymentRecord,
  SupportedCurrency
} from '~/composables/usePaymentApi'
import { currencySymbol, formatTokenAmount } from '~/composables/usePaymentApi'
import { currenciesForCluster, mintForCurrency } from '~/composables/usePaymentProgram'
import { CARD_DESIGNS, type CardDesign } from '~/utils/cardDesigns'
import type { InvoiceCosts } from '~/utils/invoiceCost'
import type { CartLine, ShopWithProducts } from '~/composables/useShop'
import { cartTotal as totalCart } from '~/composables/useShop'

interface CreatedPaymentPayload {
  payment: PaymentRecord
  qrPayload: string
  priceLamports: number
  items: NonNullable<PaymentRecord['items']>
}

interface UseSellInvoiceFormOptions {
  cluster: string
  /**
   * A page made for one shop: only its menu, and it starts priced from it. Its
   * absence is what makes a page a plain payment page — no menu to pick from,
   * no basket to price, and so no shop codes either.
   */
  shopId?: string
  selectedDiscount: Ref<DiscountCode | null>
  /** Let the buyer apply one of the seller's codes on the payment page. */
  allowDiscounts: Ref<boolean>
  priceCurrency: Ref<SupportedCurrency>
  applyCreated: (created: CreatedPaymentPayload) => void
}

export function useSellInvoiceForm(options: UseSellInvoiceFormOptions) {
  const { t } = useI18n()
  const api = usePaymentApi()
  const shopApi = useShopApi()
  const { publicKey, connected } = useSolanaWallet()
  const { ensureSellerTokenAccount } = useSellerTokenAccount()
  const { currency } = useCurrencyPreference()

  // One price, in the currency the code charges: the dropdown beside the field is
  // that currency, so there is no second control to reconcile against it. The
  // seller types what they want to be paid and the customer hands over exactly it.
  const priceInput = ref('0.01')
  // The one currency this invoice charges in. Offering several prices at once
  // only hands the customer a decision; the seller decides, and the code says it.
  const priceCurrency = options.priceCurrency
  const currencyChoices = computed(() => currenciesForCluster(options.cluster))
  const currencyItems = computed(() => currencyChoices.value.map(choice => ({
    label: currencySymbol(choice),
    value: choice
  })))
  // The amount field opens a calculator: sellers often price from arithmetic
  // rather than a number they already know.
  const calculatorOpen = ref(false)
  /** The look of the page the customer opens; the default is the plain card. */
  const design = ref<CardDesign>('classic')
  const designOptions = computed(() => CARD_DESIGNS.map(value => ({
    value,
    label: t(`sell.designs.${value}`)
  })))
  const name = ref('')
  const description = ref('')
  const memo = ref('')
  const busy = ref(false)
  const failure = ref('')
  const detailsOpen = ref(false)

  /** Whether this form belongs to a shop's own payment page. */
  const shopScoped = computed(() => Boolean(options.shopId))

  // The seller's own menus, offered as taps instead of typing. Only a shop's
  // page has one, and it is read then; a plain payment page never asks.
  const shops = ref<ShopWithProducts[]>([])
  const hasProducts = computed(() => shops.value.some(shop => shop.products.length > 0))
  const cart = ref<CartLine[]>([])
  const cartTotal = computed(() => totalCart(cart.value))
  /** The currency this shop charges in: its prices are already in it. */
  const shopCurrency = computed<SupportedCurrency>(() => shops.value[0]?.currency ?? 'sol')
  const cartActive = computed(() => cart.value.length > 0)
  // An invoice is priced one of two ways: a number the seller types, or the total
  // of what they pick off a shop's menu. Most codes are the first, so the menus
  // stay folded away until a seller asks for them.
  const priceMode = ref<'amount' | 'products'>(options.shopId ? 'products' : 'amount')
  const priceModeItems = computed(() => [
    { label: t('sell.priceModes.amount'), value: 'amount' },
    { label: t('sell.priceModes.products'), value: 'products' }
  ])
  const sellingFromShops = computed(
    () => shopScoped.value && priceMode.value === 'products'
  )
  // A code prices a basket, so the discount sections belong to a payment built
  // from the shop's products. With an empty tray — or a typed price — there is
  // nothing for a percentage to price, and the fields would only ask a question
  // that cannot apply to the code being made.
  const discountAllowed = computed(() => sellingFromShops.value && cartActive.value)
  // A buyer's own code prices a basket too, and never sits beside a code the
  // seller already attached.
  const canInviteBuyerCodes = computed(
    () => discountAllowed.value && !options.selectedDiscount.value
  )

  /**
   * The typed price in what the code charges: lamports for SOL, the token's own
   * base units for a stablecoin. A shop's cart sets it too — its products are
   * priced in lamports, which is why picking from a menu pins the currency to SOL.
   */
  const priceUnits = computed(() => {
    if (sellingFromShops.value) return cartTotal.value
    const typed = Number(priceInput.value)
    if (!Number.isFinite(typed) || typed <= 0) return 0
    return priceCurrency.value === 'sol'
      ? solToLamports(typed)
      : Math.round(typed * 1_000_000)
  })

  /** The invoice's price in its own base units — exactly what the code charges. */
  const invoiceOptions = computed(() => [
    { currency: priceCurrency.value, amount: priceUnits.value }
  ])

  /** A price in the money the customer will hand over. */
  function priceLabel(option: { currency: SupportedCurrency, amount: number }): string {
    return option.currency === 'sol'
      ? `${lamportsToSol(option.amount)} SOL`
      : `${formatTokenAmount(option.amount, 6)} ${currencySymbol(option.currency)}`
  }

  /** The price shown next to the form, and the same price after a carried code. */
  const priceSummary = computed(() => priceLabel(invoiceOptions.value[0]!))

  // What the seller actually receives, quoted from the fee policy on chain: a
  // change to `fee_bps` shows up here without a release, and a buyer-paid fee
  // leaves the net at the full price.
  const { estimateInvoiceCosts } = useInvoiceCosts()
  const netQuote = ref<InvoiceCosts | null>(null)

  async function refreshNetReceive() {
    const amount = priceUnits.value
    const currentCurrency = priceCurrency.value
    if (amount <= 0) {
      netQuote.value = null
      return
    }
    // The mint is what selects the currency's flat: quoting without it would
    // show a seller net that the token path then charges 89,400 base units for.
    const mint = currentCurrency === 'sol'
      ? undefined
      : mintForCurrency(currentCurrency, options.cluster)?.toString()
    try {
      netQuote.value = await estimateInvoiceCosts(amount, currentCurrency, {
        mint,
        cluster: options.cluster
      })
    } catch {
      // No policy, no promise: a stale number here would be a lie.
      netQuote.value = null
    }
  }

  const discountedPrice = computed(() => {
    const option = invoiceOptions.value[0]!
    if (!options.selectedDiscount.value) return priceLabel(option)
    return priceLabel({
      currency: option.currency,
      amount: discountedLamports(option.amount, options.selectedDiscount.value.percent_off)
    })
  })

  /** The price field sets the invoice, and the dropdown says which currency it charges. */
  const amountHelp = computed(() => {
    if (cartActive.value) return t('sell.amountHelpCart')
    if (sellingFromShops.value) return t('sell.amountHelpProducts')
    return t('sell.amountHelpTyped', { currency: currencySymbol(priceCurrency.value) })
  })

  // Picking products decides the price, and the shop decides the currency: the
  // invoice has to charge exactly what the receipt adds up to, in the unit the
  // menu is written in, or the code would say one price and charge another.
  watch(cartTotal, (total) => {
    if (total <= 0) return
    const shop = shopCurrency.value
    priceCurrency.value = shop
    priceInput.value = shop === 'sol'
      ? String(Number((total / 1_000_000_000).toFixed(9)))
      : formatTokenAmount(total, 6)
  })

  // Switching back to a typed amount drops the tray: a cart left behind would
  // price the invoice from items nobody can see any more. A chosen code stays:
  // it prices whatever is charged, basket or number.
  watch(priceMode, (mode) => {
    if (mode !== 'amount') return
    cart.value = []
  })

  // Quoting reads the chain, so the first one waits for the browser: the server
  // render has no policy to read.
  onMounted(() => {
    void refreshNetReceive()
  })
  watch([priceUnits, priceCurrency], () => {
    void refreshNetReceive()
  })

  const canCreate = computed(() =>
    connected.value
    && !busy.value
    && priceUnits.value > 0
    && invoiceOptions.value.every(option => option.amount > 0)
  )

  function applyAmount(amount: string) {
    priceInput.value = amount
  }

  /**
   * Stamps the seller's own choices into the invoice code: the currency the
   * buyer reads prices in, and the card design the page opens with — on any
   * phone, signed in or not. The API builds the rest of the payload; these two
   * fields are the seller's, so they are added here.
   */
  function withSellerDetails(payloadJson: string): string {
    try {
      return JSON.stringify({
        ...JSON.parse(payloadJson),
        currency: currency.value,
        design: design.value
      })
    } catch {
      return payloadJson
    }
  }

  /** The menus, if this account has any. Shops that fail to load must never stop
   *  a seller from typing an amount the way they always could. */
  async function loadShop() {
    if (!options.shopId) return
    try {
      shops.value = [await shopApi.get(options.shopId)]
      // This page charges in the shop's currency, never in whatever the seller
      // last typed on the plain payment page.
      priceCurrency.value = shops.value[0]?.currency ?? 'sol'
    } catch {
      shops.value = []
    }
  }

  async function createPayment() {
    failure.value = ''

    const seller = publicKey.value
    if (!seller) {
      failure.value = t('sell.connectFirst')
      return
    }
    if (!Number.isFinite(priceUnits.value) || priceUnits.value <= 0) {
      failure.value = t('sell.enterPrice')
      return
    }
    if (memo.value.length > 96) {
      failure.value = t('sell.memoTooLong')
      return
    }
    if (name.value.trim().length > 64) {
      failure.value = t('sell.nameTooLong')
      return
    }
    if (description.value.trim().length > 160) {
      failure.value = t('sell.descriptionTooLong')
      return
    }

    busy.value = true
    try {
      // A token invoice can only pay into an account the seller already owns, so
      // theirs is opened first — one signature, paid by the seller, before a code
      // exists to be scanned. A SOL invoice needs nothing.
      try {
        await ensureSellerTokenAccount(seller, priceCurrency.value, options.cluster)
      } catch {
        throw new Error(t('sell.tokenAccountFailure'))
      }

      // Nothing else to sign: derive the invoice address from the amount locally,
      // then record the details. The customer's payment funds the account.
      const reference = createReference()

      // The API derives the invoice address from this price, so the code the
      // customer scans names exactly the currency and amount it charges.
      const created = await api.createPayment({
        seller,
        reference,
        options: invoiceOptions.value,
        memo: memo.value,
        name: name.value.trim(),
        description: description.value.trim(),
        items: cart.value.map(line => ({
          name: line.product.name,
          quantity: line.quantity,
          amount_lamports: line.product.price_lamports
        })),
        // A code the seller picked is stamped into the invoice; otherwise the
        // invoice may invite the buyer to bring one of their own.
        discount_code: discountAllowed.value ? options.selectedDiscount.value?.code : undefined,
        allow_discounts: canInviteBuyerCodes.value && options.allowDiscounts.value,
        cluster: options.cluster
      })

      options.applyCreated({
        payment: created.payment,
        qrPayload: withSellerDetails(created.qrPayload),
        priceLamports: priceUnits.value,
        items: created.payment.items ?? []
      })
      // The invoice is made; the next one starts from an empty tray.
      cart.value = []
    } catch (error) {
      failure.value = error instanceof Error ? error.message : t('sell.createFailure')
    } finally {
      busy.value = false
    }
  }

  function clearFailure() {
    failure.value = ''
  }

  return {
    shopScoped,
    priceInput,
    priceCurrency,
    currencyItems,
    calculatorOpen,
    design,
    designOptions,
    name,
    description,
    memo,
    busy,
    failure,
    detailsOpen,
    shops,
    hasProducts,
    cart,
    cartActive,
    priceMode,
    priceModeItems,
    sellingFromShops,
    discountAllowed,
    canInviteBuyerCodes,
    priceSummary,
    netQuote,
    discountedPrice,
    amountHelp,
    canCreate,
    applyAmount,
    loadShop,
    createPayment,
    clearFailure
  }
}
