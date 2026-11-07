<script setup lang="ts">
useSeoMeta({
  title: 'Learn — Payvand',
  description: 'Crypto explained to someone who has never used it: how a blockchain works, what Solana is, what a wallet does, and how to stay safe. No prior knowledge assumed.'
})

// Reading order is the `order` field, not the filenames, so guides can be
// slotted in without renaming anything.
const { data: guides } = await useAsyncData('learn-guides', () =>
  queryCollection('learn').order('order', 'ASC').all()
)

const groups = computed(() => [
  {
    key: 'basics',
    title: 'Start here: the ideas',
    description: 'What money on a blockchain actually is, in the order that makes sense. No prior knowledge assumed.',
    icon: 'i-lucide-book-open',
    items: (guides.value ?? []).filter(guide => guide.category === 'basics')
  },
  {
    key: 'solana',
    title: 'Solana, in plain words',
    description: 'The network under Payvand: why it is fast, what a payment costs, and when it is really done.',
    icon: 'i-lucide-zap',
    items: (guides.value ?? []).filter(guide => guide.category === 'solana')
  },
  {
    key: 'payvand',
    title: 'Using Payvand',
    description: 'The monthly allowance, the practice money, and what each button on the screen does.',
    icon: 'i-lucide-qr-code',
    items: (guides.value ?? []).filter(guide => guide.category === 'payvand')
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
      description="Short guides for someone who has never used a wallet. Start with what a blockchain is, then what Solana does, then how this app and its allowance work — and read the safety guides before you use real money."
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
