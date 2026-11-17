// @vitest-environment happy-dom
import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'
import { defineComponent, h } from 'vue'
import CopyField from './CopyField.vue'
import SignatureProof from './SignatureProof.vue'

/**
 * The signature is the buyer's only proof that the payment is on chain, so these
 * cases mount the line they are given: what it shows, that the copy hands over
 * the whole signature rather than the shortened text, and that the explorer link
 * names the cluster the payment landed on.
 */
const SIGNATURE = '5J8xRn7NvYhkhCLZ8HHVwU5CTK8rQuTgYFiKn9bJvZ8HwGaP3wWiHjFx4dwm1x3Kk4vKcBXKFuXsRXS1QqTqC3o'

/** UButton renders a `<button>`, or an `<a>` when it is given a `to`. */
const UButtonStub = defineComponent({
  inheritAttrs: false,
  props: { to: { type: String, default: '' } },
  setup(props, { attrs }) {
    return () => (props.to ? h('a', { ...attrs, href: props.to }) : h('button', attrs))
  }
})

function mountProof(props: Record<string, unknown> = {}) {
  return mount(SignatureProof, {
    props: {
      signature: SIGNATURE,
      cluster: 'devnet',
      label: 'Signature',
      copyLabel: 'Copy signature',
      copiedLabel: 'Copied',
      explorerLabel: 'View on explorer',
      ...props
    },
    // Nuxt auto-imports `CopyField` in the app; a unit mount has to be told.
    global: { stubs: { UButton: UButtonStub }, components: { CopyField } }
  })
}

function stubClipboard() {
  const writeText = vi.fn(async () => {})
  Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true })
  return writeText
}

describe('SignatureProof', () => {
  it('shows the ends of the signature and keeps the whole one for the tooltip', () => {
    const wrapper = mountProof()

    expect(wrapper.text()).toContain(`${SIGNATURE.slice(0, 8)}…${SIGNATURE.slice(-8)}`)
    expect(wrapper.find('[title]').attributes('title')).toBe(SIGNATURE)
  })

  it('copies the whole signature, not the shortened text', async () => {
    const writeText = stubClipboard()
    const wrapper = mountProof()
    const copyButton = wrapper.get('button[aria-label="Copy signature"]')

    await copyButton.trigger('click')

    expect(writeText).toHaveBeenCalledWith(SIGNATURE)
    // The button says it worked, so a second press is not a guess.
    expect(wrapper.get('button').attributes('aria-label')).toBe('Copied')
  })

  it('links to the explorer for the cluster the payment landed on', () => {
    const link = mountProof().get('a')

    expect(link.attributes('href'))
      .toBe(`https://explorer.solana.com/tx/${SIGNATURE}?cluster=devnet`)
    expect(link.attributes('target')).toBe('_blank')
    expect(link.attributes('rel')).toBe('noopener noreferrer')
  })

  it('offers no link where the explorer cannot answer, but still copies', () => {
    // Localnet has no public explorer; the signature stays copyable.
    const wrapper = mountProof({ cluster: 'localnet' })

    expect(wrapper.find('a').exists()).toBe(false)
    expect(wrapper.find('button[aria-label="Copy signature"]').exists()).toBe(true)
  })
})
