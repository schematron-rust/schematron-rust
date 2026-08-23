<script lang="ts">
  // Lily Design System — SectionHeading
  //
  // Local stand-in; see ./README.md. Faithful to the upstream component:
  // a <header> carrying the `section-heading` hook, an optional eyebrow, a
  // heading at the requested level, and an optional subtitle.
  //
  // Props:
  //   class — string, optional. CSS class name appended to the base hook.
  //   heading — string, required. The main heading text.
  //   eyebrow — string, optional. Text rendered above the heading.
  //   subtitle — string, optional. Text rendered below the heading.
  //   level — 2 | 3 | 4 | 5 | 6, default 2. Heading level.

  let {
    class: className = '',
    heading,
    eyebrow = undefined,
    subtitle = undefined,
    level = 2,
    ...restProps
  }: {
    class?: string;
    heading: string;
    eyebrow?: string;
    subtitle?: string;
    level?: 2 | 3 | 4 | 5 | 6;
    [key: string]: unknown;
  } = $props();
</script>

<header class={`section-heading ${className}`} {...restProps}>
  {#if eyebrow}
    <p class="section-heading-eyebrow">{eyebrow}</p>
  {/if}
  {#if level === 2}
    <h2 class="section-heading-heading">{heading}</h2>
  {:else if level === 3}
    <h3 class="section-heading-heading">{heading}</h3>
  {:else if level === 4}
    <h4 class="section-heading-heading">{heading}</h4>
  {:else if level === 5}
    <h5 class="section-heading-heading">{heading}</h5>
  {:else}
    <h6 class="section-heading-heading">{heading}</h6>
  {/if}
  {#if subtitle}
    <p class="section-heading-subtitle">{subtitle}</p>
  {/if}
</header>
