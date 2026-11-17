// @vitest-environment happy-dom
import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'
import { defineComponent, h } from 'vue'
import CopyField from './CopyField.vue'

/**
 * One row is shared by the signature, the reference code and the wallet that paid,
 * so these cases pin what all three get: a shortened line that copies the whole
 * value, a short value left alone, and the full text on paper where nothing can
 * be clicked.
 */
const WALLET = '9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin'

/** UButton renders a `<button>`, or an `<a>` when it is given a `to`. */
const UButtonStub = defineComponent({
  inheritAttrs: false,
  props: { to: { type: String, default: '' } },
  setup(props, { attrs }) {
    return () => (props.to ? h('a', { ...attrs, href: props.to }) : h('button', attrs))
  }
})

function mountField(props: Record<string, unknown> = {}) {
  return mount(CopyField, {
    props: {
      label: 'Paid by',
      value: WALLET,
      keep: 6,
      copyLabel: 'Copy wallet',
      copiedLabel: 'Copied',
      ...props
    },
    global: { stubs: { UButton: UButtonStub } }
  })
}

function stubClipboard() {
  const writeText = vi.fn(async () => {})
  Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true })
  return writeText
}

describe('CopyField', () => {
  it('shortens a long value and keeps the whole one for the tooltip', () => {
    const wrapper = mountField()

    expect(wrapper.text()).toContain(`${WALLET.slice(0, 6)}…${WALLET.slice(-6)}`)
    expect(wrapper.find('[title]').attributes('title')).toBe(WALLET)
  })

  it('shows a value short enough to fit whole', () => {
    // An eight-character reference code has nothing worth hiding.
    const wrapper = mountField({ value: '7Fq2Pm9K', keep: 0 })

    expect(wrapper.text()).toContain('7Fq2Pm9K')
    expect(wrapper.text()).not.toContain('…')
  })

  it('copies the whole value, not the shortened text', async () => {
    const writeText = stubClipboard()
    const wrapper = mountField()

    await wrapper.get('button[aria-label="Copy wallet"]').trigger('click')

    expect(writeText).toHaveBeenCalledWith(WALLET)
    // The button says it worked, so a second press is not a guess.
    expect(wrapper.get('button').attributes('aria-label')).toBe('Copied')
  })

  it('prints the whole value and drops the buttons', () => {
    const wrapper = mountField()

    // Paper has nothing to click: the full value sits in the span the print view
    // shows, and every control is hidden on paper.
    const printed = wrapper.findAll('span').find(span => span.text() === WALLET)
    expect(printed?.classes()).toContain('print:inline')
    expect(wrapper.get('div.ml-auto').classes()).toContain('print:hidden')
  })
})
