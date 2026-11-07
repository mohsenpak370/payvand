<script setup lang="ts">
const route = useRoute()

const { data: guide } = await useAsyncData(route.path, () =>
  queryCollection('learn').path(route.path).first()
)

if (!guide.value) {
  throw createError({ statusCode: 404, statusMessage: 'Guide not found', fatal: true })
}

// Previous/next inside the collection, which is what makes the guides read as
// a course rather than a pile of pages.
const { data: surround } = await useAsyncData(`${route.path}-surround`, () =>
  queryCollectionItemSurroundings('learn', route.path, { fields: ['description'] })
)

useSeoMeta({
  title: `${guide.value.title} — Payvand`,
  description: guide.value.description
})
</script>

<template>
  <UContainer
    v-if="guide"
    class="py-8 lg:py-12"
  >
    <!--
      UPage lays out columns but adds no gutter of its own, so the container
      above supplies the padding and max width every other page gets from
      UPageHero / UPageSection.
    -->
    <UPage>
      <!-- Back to the index, above the title rather than under it: a reader
           who arrived from a search result should be able to leave the way
           they came without reading the article first. Styled like the
           header's own text links so it reads as navigation, not an action. -->
      <NuxtLink
        to="/learn"
        class="text-muted hover:text-primary inline-flex items-center gap-1.5 self-start text-sm font-medium underline-offset-4 hover:underline"
      >
        <UIcon
          name="i-lucide-arrow-left"
          class="size-4 shrink-0"
        />
        All guides
      </NuxtLink>

      <UPageHeader
        :title="guide.title"
        :description="guide.description"
      >
        <!-- Same label as the card on the index: this one assumes more. -->
        <template
          v-if="guide.level === 'technical'"
          #headline
        >
          <UBadge
            label="Technical discussion"
            icon="i-lucide-cpu"
            color="neutral"
            variant="subtle"
          />
        </template>
      </UPageHeader>

      <UPageBody class="max-w-3xl">
        <!-- Prose components come from @nuxt/ui, so headings, lists and quotes
             pick up the app's own typography and colours. -->
        <ContentRenderer :value="guide" />

        <UContentSurround :surround="surround ?? []" />
      </UPageBody>
    </UPage>
  </UContainer>
</template>
