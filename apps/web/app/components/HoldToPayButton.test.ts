// @vitest-environment happy-dom
import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'
import { defineComponent, h } from 'vue'
import HoldToPayButton from './HoldToPayButton.vue'

/**
 * The button's press is the whole consent for an allowance payment, so these
 * cases mount it and drive real pointer and keyboard events at the element Vue
 * put the handlers on. What the button cannot own — the count and the chain —
 * goes up as `start`/`stop`, so a press that should survive shows up as no
 * `stop` and a press that should end shows up as one.
 */

/** A 200×40 button at (100, 100): every slop case is measured from these edges. */
const RECT = {
  x: 100,
  y: 100,
  left: 100,
  top: 100,
  right: 300,
  bottom: 140,
  width: 200,
  height: 40,
  toJSON: () => ({})
} as DOMRect

/** UButton renders a `<button>`; a bare one is enough to bind the handlers to. */
const UButtonStub = defineComponent({
  inheritAttrs: false,
  setup(_props, { attrs, slots }) {
    return () => h('button', attrs, slots.default?.())
  }
})

function mountButton(props: Record<string, unknown> = {}) {
  const wrapper = mount(HoldToPayButton, {
    props: {
      label: 'Hold to pay 0.10 SOL',
      holdingLabel: 'Keep holding',
      holding: false,
      progress: 0,
      loading: false,
      disabled: false,
      ...props
    },
    global: { stubs: { UButton: UButtonStub } }
  })
  const button = wrapper.get('button')
  vi.spyOn(button.element, 'getBoundingClientRect').mockReturnValue(RECT)
  const capture = vi.fn<(pointerId: number) => void>()
  Object.defineProperty(button.element, 'setPointerCapture', { value: capture, configurable: true })
  return { wrapper, button, capture }
}

function press(element: Element, type: string, init: PointerEventInit = {}) {
  element.dispatchEvent(
    new PointerEvent(type, { bubbles: true, cancelable: true, pointerId: 1, isPrimary: true, button: 0, ...init })
  )
}

describe('HoldToPayButton', () => {
  it('starts on the primary press and takes the pointer capture', () => {
    const { wrapper, button, capture } = mountButton()

    press(button.element, 'pointerdown', { clientX: 200, clientY: 120 })

    expect(wrapper.emitted('start')).toHaveLength(1)
    expect(capture).toHaveBeenCalledWith(1)
    expect(wrapper.emitted('stop')).toBeUndefined()
  })

  it('keeps the press while the pointer drifts inside the slop', () => {
    const { wrapper, button } = mountButton()
    press(button.element, 'pointerdown', { clientX: 200, clientY: 120 })

    // The capture means these never reach the button as a `pointerleave`, and a
    // finger's width of drift off either edge is still the same press.
    press(button.element, 'pointermove', { clientX: 200 - 32, clientY: 120 })
    press(button.element, 'pointermove', { clientX: 200, clientY: 140 + 32 })

    expect(wrapper.emitted('stop')).toBeUndefined()
    expect(wrapper.emitted('start')).toHaveLength(1)
  })

  it('ends the press when it is dragged away, and only once', () => {
    const { wrapper, button } = mountButton()
    press(button.element, 'pointerdown', { clientX: 200, clientY: 120 })

    press(button.element, 'pointermove', { clientX: 200, clientY: 140 + 33 })

    expect(wrapper.emitted('stop')).toHaveLength(1)
    // Past the press: moving further must not end a second hold.
    press(button.element, 'pointermove', { clientX: 200, clientY: 400 })
    expect(wrapper.emitted('stop')).toHaveLength(1)
  })

  it('ignores a pointer that is not the one holding the button', () => {
    const { wrapper, button } = mountButton()
    press(button.element, 'pointerdown', { clientX: 200, clientY: 120 })

    // A second finger's drag is not this press, however far it goes.
    press(button.element, 'pointermove', { pointerId: 2, clientX: 900, clientY: 900 })

    expect(wrapper.emitted('stop')).toBeUndefined()
  })

  it('ends the press when the pointer is released or the capture is lost', () => {
    for (const type of ['pointerup', 'pointercancel', 'lostpointercapture']) {
      const { wrapper, button } = mountButton()
      press(button.element, 'pointerdown', { clientX: 200, clientY: 120 })
      press(button.element, type, { clientX: 200, clientY: 120 })
      expect(wrapper.emitted('stop'), type).toHaveLength(1)
    }
  })

  it('does not start a second press on a repeated keydown', async () => {
    const { wrapper, button } = mountButton()

    await button.trigger('keydown', { key: ' ' })
    expect(wrapper.emitted('start')).toHaveLength(1)

    // A held key repeats; the checkout sets `holding` on the press it accepted,
    // and the repeat must not restart the count.
    await wrapper.setProps({ holding: true })
    await button.trigger('keydown', { key: ' ' })
    await button.trigger('keydown', { key: 'Enter' })
    expect(wrapper.emitted('start')).toHaveLength(1)

    // Letting go ends it, and the next press is a new one.
    await button.trigger('keyup', { key: ' ' })
    expect(wrapper.emitted('stop')).toHaveLength(1)
    await wrapper.setProps({ holding: false })
    await button.trigger('keydown', { key: 'Enter' })
    expect(wrapper.emitted('start')).toHaveLength(2)
  })

  it('does not start while a hold is already running', () => {
    const { wrapper, button } = mountButton({ holding: true })

    press(button.element, 'pointerdown', { clientX: 200, clientY: 120 })

    expect(wrapper.emitted('start')).toBeUndefined()
  })

  it('refuses the presses that are not a decision to pay', () => {
    const { wrapper, button } = mountButton()

    press(button.element, 'pointerdown', { button: 2, clientX: 200, clientY: 120 })
    press(button.element, 'pointerdown', { isPrimary: false, clientX: 200, clientY: 120 })

    expect(wrapper.emitted('start')).toBeUndefined()
  })
})
