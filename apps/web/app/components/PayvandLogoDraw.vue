<!--
  PayvandLogoDraw.vue: Payvand app icon, drawn on and then lit.
    1. The P and the connecting ribbon are drawn together from the foot of the stem and finish together.
    2. The V's left arm comes out from under the bowl, then the V's right arm is drawn up to the top right.
    3. A light travels the ribbon, under the bowl and along the V to its tip, where a ring ripples out.
  Vue 3.5+ (uses useId). No dependencies, no text, transparent outside the icon tile.

  Props   size (px or any CSS length, default 256) · loop (default false) · interval (ms between replays, default 9000)
  Expose  replay()  restart the animation from a parent through a template ref
-->
<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch, useId } from 'vue'

const props = withDefaults(
  defineProps<{ size?: number | string, loop?: boolean, interval?: number }>(),
  { size: 256, loop: false, interval: 9000 }
)

const uid = `pv${useId()}`.replace(/[^a-zA-Z0-9_-]/g, '')
const px = computed(() => (typeof props.size === 'number' ? `${props.size}px` : props.size))

const run = ref(0) // changing the key on <svg> re-mounts it and restarts every CSS animation
const replay = () => {
  run.value++
}
defineExpose({ replay })

let timer: ReturnType<typeof setInterval> | undefined
const sync = () => {
  if (timer) clearInterval(timer)
  timer = props.loop ? setInterval(replay, Math.max(props.interval, 7600)) : undefined
}
onMounted(sync)
watch(() => [props.loop, props.interval], sync)
onBeforeUnmount(() => timer && clearInterval(timer))
</script>

<template>
  <svg
    :key="run"
    class="pv-icon"
    :style="{ width: px, height: px }"
    xmlns="http://www.w3.org/2000/svg"
    viewBox="177 172.5 899 899"
    role="img"
    aria-label="Payvand"
  >
    <defs>

      <linearGradient
        :id="`${uid}-g_vr`"
        gradientUnits="userSpaceOnUse"
        x1="750.7"
        y1="735.2"
        x2="942.5"
        y2="486.8"
      ><stop
        offset="0.000"
        stop-color="#516bfd"
      /><stop
        offset="0.167"
        stop-color="#4d72fd"
      /><stop
        offset="0.278"
        stop-color="#497afd"
      /><stop
        offset="0.389"
        stop-color="#4482fd"
      /><stop
        offset="0.500"
        stop-color="#3f8bfd"
      /><stop
        offset="0.611"
        stop-color="#3b93fd"
      /><stop
        offset="0.722"
        stop-color="#399bfd"
      /><stop
        offset="0.833"
        stop-color="#38a1fd"
      /><stop
        offset="1.000"
        stop-color="#37a8fd"
      /></linearGradient>

      <linearGradient
        :id="`${uid}-g_vl`"
        gradientUnits="userSpaceOnUse"
        x1="766.0"
        y1="813.7"
        x2="650.9"
        y2="643.4"
      ><stop
        offset="0.000"
        stop-color="#3ebffd"
      /><stop
        offset="0.167"
        stop-color="#3ebefd"
      /><stop
        offset="0.278"
        stop-color="#3bbafd"
      /><stop
        offset="0.389"
        stop-color="#39b2fd"
      /><stop
        offset="0.500"
        stop-color="#36a9fd"
      /><stop
        offset="0.611"
        stop-color="#339cfd"
      /><stop
        offset="0.722"
        stop-color="#308efd"
      /><stop
        offset="0.833"
        stop-color="#307bfd"
      /><stop
        offset="1.000"
        stop-color="#2d5ee9"
      /></linearGradient>

      <linearGradient
        :id="`${uid}-g_p`"
        gradientUnits="userSpaceOnUse"
        x1="344.0"
        y1="444.7"
        x2="605.7"
        y2="684.9"
      ><stop
        offset="0.000"
        stop-color="#3dbcfd"
      /><stop
        offset="0.167"
        stop-color="#3bb4fd"
      /><stop
        offset="0.278"
        stop-color="#37a8fd"
      /><stop
        offset="0.389"
        stop-color="#3698fd"
      /><stop
        offset="0.500"
        stop-color="#3e88fd"
      /><stop
        offset="0.611"
        stop-color="#4a7afd"
      /><stop
        offset="0.722"
        stop-color="#526dfc"
      /><stop
        offset="0.833"
        stop-color="#3d5ef8"
      /><stop
        offset="1.000"
        stop-color="#314cf2"
      /></linearGradient>

      <linearGradient
        :id="`${uid}-g_rb`"
        gradientUnits="userSpaceOnUse"
        x1="612.2"
        y1="617.7"
        x2="403.4"
        y2="678.0"
      ><stop
        offset="0.000"
        stop-color="#3ec1fd"
      /><stop
        offset="0.167"
        stop-color="#3dbefd"
      /><stop
        offset="0.278"
        stop-color="#39b3fd"
      /><stop
        offset="0.389"
        stop-color="#35a4fd"
      /><stop
        offset="0.500"
        stop-color="#3393fd"
      /><stop
        offset="0.611"
        stop-color="#3880fd"
      /><stop
        offset="0.722"
        stop-color="#416bfc"
      /><stop
        offset="0.833"
        stop-color="#4157f7"
      /><stop
        offset="1.000"
        stop-color="#3149e2"
      /></linearGradient>

      <clipPath :id="`${uid}-c_vl`">
        <path d="M757.0 824.2 L746.0 826.4 L739.0 826.5 L725.0 823.2 L718.0 820.2 L711.7 816.0 L699.6 804.0 L612.9 660.0 L612.9 658.0 L615.0 656.8 L629.0 655.4 L643.0 651.2 L658.0 643.2 L675.4 627.0 L686.0 610.8 L688.4 612.0 L697.2 623.0 L741.0 694.2 L744.0 695.7 L783.2 760.0 L788.2 771.0 L792.0 791.0 L787.2 800.0 L777.0 812.4 L768.0 819.2Z" /></clipPath>

      <filter
        :id="`${uid}-soft`"
        x="-20%"
        y="-20%"
        width="140%"
        height="140%"
      ><feGaussianBlur stdDeviation="9" /></filter>

      <mask
        :id="`${uid}-m_p`"
        maskUnits="userSpaceOnUse"
        x="0"
        y="0"
        width="1254"
        height="1254"
      >
        <path
          class="wipe wp"
          pathLength="1"
          d="M377 850 V455 H555 C615 455 650 505 650 555 C650 590 625 612 590 612 L520 612"
          fill="none"
          stroke="#fff"
          stroke-width="170"
          stroke-linejoin="round"
        /></mask>
      <mask
        :id="`${uid}-m_rb`"
        maskUnits="userSpaceOnUse"
        x="0"
        y="0"
        width="1254"
        height="1254"
      >
        <path
          class="wipe wr"
          pathLength="1"
          d="M385 780 L510 655 C530 635 548 612 600 604 L630 604"
          fill="none"
          stroke="#fff"
          stroke-width="170"
          stroke-linejoin="round"
        /></mask>
      <mask
        :id="`${uid}-m_vl`"
        maskUnits="userSpaceOnUse"
        x="0"
        y="0"
        width="1254"
        height="1254"
      >
        <path
          class="wipe wv"
          pathLength="1"
          d="M645 600 L772 826"
          fill="none"
          stroke="#fff"
          stroke-width="170"
          stroke-linejoin="round"
        /></mask>
      <mask
        :id="`${uid}-m_vr`"
        maskUnits="userSpaceOnUse"
        x="0"
        y="0"
        width="1254"
        height="1254"
      >
        <path
          class="wipe wx"
          pathLength="1"
          d="M731 815 L934 455"
          fill="none"
          stroke="#fff"
          stroke-width="170"
          stroke-linejoin="round"
        /></mask>
      <radialGradient :id="`${uid}-halo`"><stop
        offset="0"
        stop-color="#7fdcff"
        stop-opacity=".95"
      /><stop
        offset=".45"
        stop-color="#3dc0ff"
        stop-opacity=".45"
      /><stop
        offset="1"
        stop-color="#3dc0ff"
        stop-opacity="0"
      /></radialGradient>
      <mask
        :id="`${uid}-nodot`"
        maskUnits="userSpaceOnUse"
        x="0"
        y="0"
        width="1254"
        height="1254"
      ><rect
         width="1254"
         height="1254"
         fill="#fff"
       />
        <path
          d="M339.0 821.2 L334.0 814.5 L331.5 807.0 L331.5 451.0 L333.8 439.0 L339.6 427.0 L353.0 414.8 L359.0 411.8 L371.0 408.5 L572.0 408.5 L591.0 412.8 L615.0 423.8 L635.3 437.0 L653.0 452.5 L669.2 473.0 L683.2 499.0 L692.4 528.0 L695.5 547.0 L695.5 573.0 L692.2 593.0 L687.0 609.0 L687.1 613.0 L678.0 628.3 L665.0 641.4 L657.0 647.2 L645.0 653.2 L636.0 656.2 L615.0 659.4 L609.0 657.5 L570.0 657.5 L552.0 661.5 L550.8 660.0 L551.8 658.0 L575.3 642.0 L587.4 631.0 L596.7 618.0 L604.2 600.0 L606.5 587.0 L606.5 566.0 L605.0 561.0 L605.5 556.0 L604.4 549.0 L599.7 535.0 L590.4 519.0 L580.0 508.6 L573.0 503.8 L558.0 497.6 L542.0 496.5 L535.0 497.5 L522.0 496.5 L514.0 497.5 L506.0 496.6 L498.0 497.5 L489.0 496.7 L450.0 497.5 L436.0 496.5 L423.5 498.0 L422.5 501.0 L422.5 654.0 L421.5 661.0 L421.5 774.0 L422.5 780.0 L422.5 804.0 L421.2 811.0 L416.2 820.0 L410.5 824.0 L404.0 826.4 L351.0 826.5 L345.6 825.0Z"
          fill="#000"
        /></mask></defs>

    <path
      id="tile"
      class="piece"
      fill="#080e20"
      d="M981.0 1023.5 L971.0 1031.2 L954.0 1042.2 L939.0 1050.3 L923.0 1057.2 L900.0 1064.5 L886.0 1067.5 L864.0 1070.5 L852.0 1070.5 L847.0 1071.5 L405.0 1071.5 L401.0 1070.5 L390.0 1070.5 L387.0 1069.5 L373.0 1068.5 L358.0 1065.5 L346.0 1062.2 L321.0 1053.2 L298.0 1041.2 L282.7 1031.0 L267.0 1018.5 L250.5 1002.0 L240.0 989.3 L229.8 975.0 L220.8 960.0 L211.8 942.0 L206.8 930.0 L200.8 912.0 L195.5 891.0 L192.5 872.0 L192.5 864.0 L191.5 860.0 L191.5 842.0 L190.5 838.0 L190.5 414.0 L191.5 409.0 L191.5 385.0 L192.5 382.0 L192.5 374.0 L194.5 360.0 L198.8 340.0 L208.8 310.0 L219.8 287.0 L234.8 263.0 L251.5 242.0 L267.0 226.5 L283.7 213.0 L296.0 204.8 L308.0 197.8 L325.0 189.8 L341.0 183.8 L359.0 178.8 L376.0 175.5 L389.0 174.5 L392.0 173.5 L403.0 173.5 L408.0 172.5 L846.0 172.5 L850.0 173.5 L862.0 173.5 L865.0 174.5 L871.0 174.5 L889.0 177.5 L906.0 181.8 L921.0 186.8 L933.0 191.8 L954.0 202.8 L969.3 213.0 L984.0 224.5 L1005.5 246.0 L1017.0 260.7 L1026.2 274.0 L1032.2 284.0 L1042.2 304.0 L1048.2 319.0 L1053.2 335.0 L1057.5 352.0 L1060.5 372.0 L1060.5 380.0 L1061.5 384.0 L1061.5 399.0 L1062.5 403.0 L1062.5 842.0 L1061.5 846.0 L1061.5 861.0 L1060.5 865.0 L1060.5 872.0 L1057.5 892.0 L1052.2 913.0 L1041.2 943.0 L1029.2 966.0 L1018.0 983.3 L997.5 1008.0Z"
    />

    <path
      id="vr"
      :mask="`url(#${uid}-m_vr)`"
      :fill="`url(#${uid}-g_vr)`"
      d="M790.0 790.7 L788.6 788.0 L787.2 776.0 L784.3 768.0 L743.0 699.1 L742.9 695.0 L863.0 486.5 L872.0 478.8 L878.0 476.6 L943.0 476.5 L949.0 478.3 L955.4 484.0 L958.2 489.0 L959.5 495.0 L957.3 503.0 L794.2 788.0 L792.0 790.7Z"
    />

    <path
      id="vl"
      :mask="`url(#${uid}-m_vl)`"
      :fill="`url(#${uid}-g_vl)`"
      d="M757.0 824.2 L746.0 826.4 L739.0 826.5 L725.0 823.2 L718.0 820.2 L711.7 816.0 L699.6 804.0 L612.9 660.0 L612.9 658.0 L615.0 656.8 L629.0 655.4 L643.0 651.2 L658.0 643.2 L675.4 627.0 L686.0 610.8 L688.4 612.0 L697.2 623.0 L741.0 694.2 L744.0 695.7 L783.2 760.0 L788.2 771.0 L792.0 791.0 L787.2 800.0 L777.0 812.4 L768.0 819.2Z"
    />

    <g class="shfade">
      <g :clip-path="`url(#${uid}-c_vl)`">
        <path
          d="M339.0 821.2 L334.0 814.5 L331.5 807.0 L331.5 451.0 L333.8 439.0 L339.6 427.0 L353.0 414.8 L359.0 411.8 L371.0 408.5 L572.0 408.5 L591.0 412.8 L615.0 423.8 L635.3 437.0 L653.0 452.5 L669.2 473.0 L683.2 499.0 L692.4 528.0 L695.5 547.0 L695.5 573.0 L692.2 593.0 L687.0 609.0 L687.1 613.0 L678.0 628.3 L665.0 641.4 L657.0 647.2 L645.0 653.2 L636.0 656.2 L615.0 659.4 L609.0 657.5 L570.0 657.5 L552.0 661.5 L550.8 660.0 L551.8 658.0 L575.3 642.0 L587.4 631.0 L596.7 618.0 L604.2 600.0 L606.5 587.0 L606.5 566.0 L605.0 561.0 L605.5 556.0 L604.4 549.0 L599.7 535.0 L590.4 519.0 L580.0 508.6 L573.0 503.8 L558.0 497.6 L542.0 496.5 L535.0 497.5 L522.0 496.5 L514.0 497.5 L506.0 496.6 L498.0 497.5 L489.0 496.7 L450.0 497.5 L436.0 496.5 L423.5 498.0 L422.5 501.0 L422.5 654.0 L421.5 661.0 L421.5 774.0 L422.5 780.0 L422.5 804.0 L421.2 811.0 L416.2 820.0 L410.5 824.0 L404.0 826.4 L351.0 826.5 L345.6 825.0Z"
          fill="#0a1060"
          fill-opacity=".45"
          :filter="`url(#${uid}-soft)`"
          transform="translate(4 10)"
        /></g></g>

    <path
      id="p"
      :mask="`url(#${uid}-m_p)`"
      :fill="`url(#${uid}-g_p)`"
      d="M339.0 821.2 L334.0 814.5 L331.5 807.0 L331.5 451.0 L333.8 439.0 L339.6 427.0 L353.0 414.8 L359.0 411.8 L371.0 408.5 L572.0 408.5 L591.0 412.8 L615.0 423.8 L635.3 437.0 L653.0 452.5 L669.2 473.0 L683.2 499.0 L692.4 528.0 L695.5 547.0 L695.5 573.0 L692.2 593.0 L687.0 609.0 L687.1 613.0 L678.0 628.3 L665.0 641.4 L657.0 647.2 L645.0 653.2 L636.0 656.2 L615.0 659.4 L609.0 657.5 L570.0 657.5 L552.0 661.5 L550.8 660.0 L551.8 658.0 L575.3 642.0 L587.4 631.0 L596.7 618.0 L604.2 600.0 L606.5 587.0 L606.5 566.0 L605.0 561.0 L605.5 556.0 L604.4 549.0 L599.7 535.0 L590.4 519.0 L580.0 508.6 L573.0 503.8 L558.0 497.6 L542.0 496.5 L535.0 497.5 L522.0 496.5 L514.0 497.5 L506.0 496.6 L498.0 497.5 L489.0 496.7 L450.0 497.5 L436.0 496.5 L423.5 498.0 L422.5 501.0 L422.5 654.0 L421.5 661.0 L421.5 774.0 L422.5 780.0 L422.5 804.0 L421.2 811.0 L416.2 820.0 L410.5 824.0 L404.0 826.4 L351.0 826.5 L345.6 825.0Z"
    />

    <path
      id="rb"
      :mask="`url(#${uid}-m_rb)`"
      :fill="`url(#${uid}-g_rb)`"
      d="M421.0 781.2 L419.5 776.0 L420.0 656.5 L425.0 655.4 L435.5 646.0 L487.0 595.6 L502.7 583.0 L513.0 576.7 L531.0 567.8 L543.0 563.8 L558.0 560.6 L567.0 559.5 L607.0 560.4 L608.5 564.0 L608.5 590.0 L606.2 602.0 L598.7 620.0 L589.4 633.0 L577.0 644.2 L551.0 662.2 L538.0 669.6 L425.5 779.0Z"
    />

    <g :mask="`url(#${uid}-nodot)`">
      <g
        class="dot"
        style="--lag:0.20s;--o:0.14"
      >
        <circle
          r="5"
          fill="#8fe0ff"
        /></g>
      <g
        class="dot"
        style="--lag:0.15s;--o:0.25"
      >
        <circle
          r="6"
          fill="#8fe0ff"
        /></g>
      <g
        class="dot"
        style="--lag:0.10s;--o:0.38"
      >
        <circle
          r="7"
          fill="#8fe0ff"
        /></g>
      <g
        class="dot"
        style="--lag:0.05s;--o:0.55"
      >
        <circle
          r="8"
          fill="#8fe0ff"
        /></g>
      <g
        class="dot"
        style="--lag:0s;--o:1"
      >
        <circle
          r="36"
          :fill="`url(#${uid}-halo)`"
        />
        <circle
          r="10"
          fill="#fff"
        /></g></g>
    <circle
      class="ring"
      cx="904.9"
      cy="506.8"
      r="30"
      vector-effect="non-scaling-stroke"
    /></svg>
</template>

<style scoped>
.pv-icon{display:block;overflow:visible}
.piece{animation:pv-fadein .45s ease both}
@keyframes pv-fadein{from{opacity:0}to{opacity:1}}
/* the P is drawn from the foot of its stem to the end of its bowl while, at the same moment, the ribbon is drawn from the stem's foot up to the P's top; both finish together. The V's left arm then continues out from under the bowl, and when it reaches the bottom the V's right arm is drawn up to the top right. */
.wipe{stroke-dasharray:1 1;stroke-dashoffset:1;stroke-opacity:0;animation:pv-wipe var(--d) cubic-bezier(.45,0,.25,1) var(--t) forwards}
@keyframes pv-wipe{0%{stroke-dashoffset:1;stroke-opacity:0}1%{stroke-opacity:1}100%{stroke-dashoffset:0;stroke-opacity:1}}
.wp,.wr{--t:.55s;--d:1.7s}
.wv{--t:2.15s;--d:.7s}
.wx{--t:2.85s;--d:.75s}
.shfade{opacity:0;animation:pv-fadein .4s ease 2.5s forwards}
/* once the logo is drawn, one light travels the ribbon and the V and ripples at the tip.
   The ring's stroke keeps its width in screen pixels: at the size this is shown on a
   hero card, a stroke measured in the drawing's own units came out under half a pixel
   and the ripple was there but invisible. */
.dot{offset-path:path("M396.0 745.5 C401.0 740.7 419.3 722.9 426.0 716.5 C432.7 710.1 432.7 710.2 436.0 707.0 C439.3 703.8 442.7 700.2 446.0 697.0 C449.3 693.8 452.7 690.7 456.0 687.5 C459.3 684.3 462.7 681.2 466.0 678.0 C469.3 674.8 472.7 671.2 476.0 668.0 C479.3 664.8 482.7 661.6 486.0 658.5 C489.3 655.4 492.7 652.4 496.0 649.5 C499.3 646.6 502.7 643.8 506.0 641.0 C509.3 638.2 512.7 635.5 516.0 633.0 C519.3 630.5 522.7 628.3 526.0 626.0 C529.3 623.7 532.7 621.0 536.0 619.0 C539.3 617.0 542.7 615.8 546.0 614.0 C549.3 612.2 552.7 609.7 556.0 608.0 C559.3 606.3 562.7 605.2 566.0 604.0 C569.3 602.8 572.7 601.8 576.0 600.5 C579.3 599.2 584.3 596.8 586.0 596.0 L646.1 628.0 L728.1 760.5 Q746.0 789.5 762.6 759.8 L904.9 506.8");offset-rotate:0deg;offset-distance:0%;opacity:0;animation:run 2.9s cubic-bezier(.5,0,.35,1) calc(4s + var(--lag)) forwards}
@keyframes run{0%{offset-distance:0%;opacity:0}5%{opacity:var(--o)}94%{opacity:var(--o)}100%{offset-distance:100%;opacity:0}}
.ring{fill:none;stroke:#5cc8ff;stroke-width:1.6px;opacity:0;transform-box:fill-box;transform-origin:center;animation:ring 1s cubic-bezier(.2,.6,.3,1) 6.85s forwards}
@keyframes ring{0%{transform:scale(.4);opacity:.9}100%{transform:scale(3.4);opacity:0}}
@media (prefers-reduced-motion:reduce){.dot,.ring{display:none}.piece,.shfade{animation:none;opacity:1}.wipe{animation:none;stroke-dashoffset:0;stroke-opacity:1}}
</style>
