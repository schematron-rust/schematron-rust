<script lang="ts">
  // Lily Design System — TableHead. Local stand-in; see ./README.md.
  import type { Snippet } from 'svelte';

  let {
    class: className = '',
    children,
    ...restProps
  }: {
    class?: string;
    /** TableRow elements with header cells. */
    children: Snippet;
    [key: string]: unknown;
  } = $props();
</script>

<thead class={`table-head ${className}`} {...restProps}>
  {@render children()}
</thead>
