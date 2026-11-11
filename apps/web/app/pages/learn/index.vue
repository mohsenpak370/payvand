<script setup lang="ts">
const { t } = useI18n()

useSeoMeta({
  title: t('learn.metaTitle'),
  description: t('learn.metaDescription')
})

// Reading order is the `order` field, not the filenames, so guides can be
// slotted in without renaming anything.
const { data: guides } = await useAsyncData('learn-guides', () =>
  queryCollection('learn').order('order', 'ASC').all()
)

const groups = computed(() => [
  {
    key: 'basics',
    title: t('learn.groups.basics.title'),
    description: t('learn.groups.basics.description'),
    icon: 'i-lucide-book-open',
    items: (guides.value ?? []).filter(guide => guide.category === 'basics')
  },
  {
    key: 'solana',
    title: t('learn.groups.solana.title'),
    description: t('learn.groups.solana.description'),
    icon: 'i-lucide-zap',
    items: (guides.value ?? []).filter(guide => guide.category === 'solana')
  },
  {
    key: 'payvand',
    title: t('learn.groups.payvand.title'),
    description: t('learn.groups.payvand.description'),
    icon: 'i-lucide-qr-code',
    items: (guides.value ?? []).filter(guide => guide.category === 'payvand')
  },
  {
    key: 'safety',
    title: t('learn.groups.safety.title'),
    description: t('learn.groups.safety.description'),
    icon: 'i-lucide-shield-check',
    items: (guides.value ?? []).filter(guide => guide.category === 'safety')
  },
  {
    key: 'technical',
    title: t('learn.groups.technical.title'),
    description: t('learn.groups.technical.description'),
    icon: 'i-lucide-cpu',
    items: (guides.value ?? []).filter(guide => guide.category === 'technical')
  }
])
</script>

<template>
  <div>
    <UPageHero
      :headline="$t('learn.hero.headline')"
      :title="$t('learn.hero.title')"
      :description="$t('learn.hero.description')"
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
        >
          <!-- Marks the deep-dive guides so a beginner can skip them. -->
          <template
            v-if="guide.level === 'technical'"
            #footer
          >
            <UBadge
              :label="$t('learn.technicalBadge')"
              icon="i-lucide-cpu"
              color="neutral"
              variant="subtle"
              size="sm"
            />
          </template>
        </UPageCard>
      </UPageGrid>
    </UPageSection>

    <UPageSection>
      <p class="text-muted text-sm">
        {{ $t('learn.disclaimer') }}
      </p>
    </UPageSection>
  </div>
</template>
