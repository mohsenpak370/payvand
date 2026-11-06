<script setup lang="ts">
useSeoMeta({
  title: 'Learn — Payvand',
  description: 'Plain-language guides to wallets, stablecoins and staying safe with crypto. No prior knowledge assumed.'
})

// Reading order is the `order` field, not the filenames, so guides can be
// slotted in without renaming anything.
const { data: guides } = await useAsyncData('learn-guides', () =>
  queryCollection('learn').order('order', 'ASC').all()
)

const groups = computed(() => [
  {
    key: 'basics',
    title: 'How crypto payments work',
    description: 'Start here if you have never used a wallet.',
    icon: 'i-lucide-wallet',
    items: (guides.value ?? []).filter(guide => guide.category === 'basics')
  },
  {
    key: 'safety',
    title: 'Staying safe',
    description: 'The handful of habits that prevent almost every crypto loss.',
    icon: 'i-lucide-shield-check',
    items: (guides.value ?? []).filter(guide => guide.category === 'safety')
  }
])
</script>

<template>
  <div>
    <UPageHero
      headline="Learn"
      title="Crypto, explained like a person"
      description="Short guides for people who have never used a wallet: what the keys do, why stablecoins exist, and how to keep an account that nobody can reset."
      :ui="{ title: 'leading-tight' }"
    />

    <!--
      One section per category, each a grid of the guides inside it. The whole
      card is the link, so the title and description are the only affordances
      needed.
    -->
    <UPageSection
      v-for="group in groups"
      :id="group.key"
      :key="group.key"
      :title="group.title"
      :description="group.description"
    >
      <UPageGrid>
        <UPageCard
          v-for="guide in group.items"
          :key="guide.path"
          :icon="group.icon"
          :title="guide.title"
          :description="guide.description"
          :to="guide.path"
          spotlight
        />
      </UPageGrid>
    </UPageSection>

    <UPageSection>
      <p class="text-muted text-sm">
        These guides are general education about how the tools work — not financial advice, and
        not a promise about the price of anything.
      </p>
    </UPageSection>
  </div>
</template>
