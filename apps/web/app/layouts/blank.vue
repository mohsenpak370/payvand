<template>
  <UMain :style="{ '--ui-header-height': '0px' }">
    <!--
      The chrome-less shell for pages that own the whole viewport: the payment
      sheet (`/p/[token]`, `/pay`) and the password reset form. It keeps the
      `UMain` baseline the pages were written against, with the header height
      zeroed because no header renders here.
    -->
    <slot />
  </UMain>
</template>
