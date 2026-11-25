<!--
  PayResultMark.vue: the outcome of a payment, drawn rather than dropped in.
  The circle strokes itself on, then the tick — or, for a refusal, the cross —
  draws after it, so the answer arrives in two beats.

  The drawing is the stroke-dasharray trick: each stroke gets a dash as long as
  its path and an equal offset, which hides it, then the offset animates to
  zero. 252 is the circle's circumference at r=40 rounded up; the tick is 50
  and each arm of the cross 34, both measured with getTotalLength.

  Colour comes from the parent's text colour. Size it from the outside
  (`class="size-16"`); anyone who asked their device for less motion gets the
  finished mark and no animation.
-->
<script setup lang="ts">
defineProps<{ state: 'success' | 'error' }>()
</script>

<template>
  <svg
    viewBox="0 0 100 100"
    fill="none"
    stroke="currentColor"
    stroke-width="6"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <circle
      cx="50"
      cy="50"
      r="40"
    />
    <polyline
      v-if="state === 'success'"
      points="35 50 45 60 65 40"
    />
    <g v-else>
      <line
        x1="38"
        y1="38"
        x2="62"
        y2="62"
      />
      <line
        x1="62"
        y1="38"
        x2="38"
        y2="62"
      />
    </g>
  </svg>
</template>

<style scoped>
svg {
  display: block;
  overflow: visible;
}

circle {
  stroke-dasharray: 252;
  stroke-dashoffset: 252;
  animation: mark-draw 0.5s cubic-bezier(0.34, 1.56, 0.64, 1) 0.1s forwards;
}

polyline {
  stroke-dasharray: 50;
  stroke-dashoffset: 50;
  animation: mark-draw 0.34s cubic-bezier(0.34, 1.56, 0.64, 1) 0.4s forwards;
}

g line {
  stroke-dasharray: 34;
  stroke-dashoffset: 34;
  animation: mark-draw 0.24s ease-out forwards;
}

g line:first-child {
  animation-delay: 0.38s;
}

g line:last-child {
  animation-delay: 0.5s;
}

@keyframes mark-draw {
  to {
    stroke-dashoffset: 0;
  }
}

@media (prefers-reduced-motion: reduce) {
  circle,
  polyline,
  g line {
    animation: none;
    stroke-dashoffset: 0;
  }
}
</style>
