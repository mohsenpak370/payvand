<script setup lang="ts">
import { onBeforeUnmount, onMounted } from 'vue'
import { acceptsHoldPress, strayedFromHoldBounds } from '~/utils/holdSlop'

/**
 * The allowance's pay button. Paying from the allowance sends without a wallet
 * approval, so the press is the consent: the button has to be held for the whole
 * count, and letting go sends and charges nothing.
 *
 * The count itself belongs to the checkout, which knows whether a payment can
 * still be made — `holding` and `progress` come down as props and `start`/`stop`
 * go up. What lives here is the press: which pointer is holding the button, when
 * a drift stops being that press, and every way a press can end.
 */
const props = defineProps<{
  /** What the button says, already written out: "Hold to pay 0.10 SOL". */
  label: string
  /** The same, once the count is running. */
  holdingLabel: string
  holding: boolean
  /** 0 to 1: how much of the count has run. */
  progress: number
  loading: boolean
  disabled: boolean
}>()

const emit = defineEmits<{ start: [], stop: [] }>()

/*
 * The press has to survive a finger sliding a millimetre off the edge, which is
 * why the button captures the pointer: while it holds that capture `pointerleave`
 * does not fire at all, so the hold ends when the pointer is really released and
 * not when the touch drifts.
 *
 * That also means a drag no longer cancels by itself, so `trackHold` keeps the
 * old way out: drifting within a finger's width of the button is still the same
 * press, and pulling away from it is not — `strayedFromHoldBounds` is that rule,
 * and `holdSlop.test.ts` holds both of its sides.
 *
 * Nothing here lets go of the capture: the browser does that when the pointer is
 * released, cancelled or taken away, and every one of those ends the hold below.
 * `endHold` is idempotent, so the `lostpointercapture` that follows a release
 * finds no press and changes nothing.
 */
/** The id of the pointer holding the button, while one is. */
let press: number | null = null

function startHold(event?: PointerEvent) {
  // A press already running keeps its own clock, and a right-click or a second
  // finger is not the deliberate press this button asks for.
  if (props.holding) return
  if (event && !acceptsHoldPress(event)) return
  const target = event?.currentTarget
  if (event && target instanceof Element) {
    press = event.pointerId
    try {
      target.setPointerCapture(event.pointerId)
    } catch {
      // No capture here (an older browser, or a pointer already gone): the hold
      // still works, it just stops if the pointer leaves the button.
    }
  }
  emit('start')
}

function trackHold(event: PointerEvent) {
  const target = event.currentTarget
  if (press === null || event.pointerId !== press || !(target instanceof Element)) return
  const away = strayedFromHoldBounds(target.getBoundingClientRect(), {
    x: event.clientX,
    y: event.clientY
  })
  if (away) endHold()
}

/** The end of a pointer press: abandon the hold, and let the browser drop the capture. */
function endHold() {
  press = null
  emit('stop')
}

/**
 * A press nobody is watching must not complete behind the buyer's back: losing
 * the window ends it, and so does the tab going to the background — there the
 * clock keeps running while the ticks are throttled, so the first tick after
 * coming back would see the count already done and pay at once. Both go through
 * `endHold`, which also forgets the press.
 */
function stopWhenHidden() {
  if (document.hidden) endHold()
}

onMounted(() => {
  window.addEventListener('blur', endHold)
  document.addEventListener('visibilitychange', stopWhenHidden)
})

// A hold belongs to the button on screen: the sheet closing or the invoice
// changing takes the press with it, so the clock cannot finish on its own and
// send a payment from a page nobody is looking at.
onBeforeUnmount(() => {
  window.removeEventListener('blur', endHold)
  document.removeEventListener('visibilitychange', stopWhenHidden)
  endHold()
})
</script>

<template>
  <!-- While the button holds the pointer's capture, `pointerup`,
       `pointercancel` and `lostpointercapture` are the ends it gets;
       `pointerleave` is the one that still works when the browser refused the
       capture. All four are idempotent. -->
  <UButton
    block
    icon="i-lucide-zap"
    :loading="loading"
    :disabled="disabled"
    class="relative touch-none overflow-hidden select-none"
    :aria-label="label"
    @pointerdown.prevent="startHold"
    @pointermove="trackHold"
    @pointerup="endHold"
    @pointerleave="endHold"
    @pointercancel="endHold"
    @lostpointercapture="endHold"
    @keydown.space.prevent="startHold()"
    @keydown.enter.prevent="startHold()"
    @keyup.space="endHold"
    @keyup.enter="endHold"
    @blur="endHold"
    @contextmenu.prevent
  >
    <span
      class="pointer-events-none absolute inset-y-0 start-0 bg-current/20"
      :style="{ width: `${Math.round(progress * 100)}%` }"
      aria-hidden="true"
    />
    <span class="relative">
      {{ holding ? holdingLabel : label }}
    </span>
  </UButton>
</template>
