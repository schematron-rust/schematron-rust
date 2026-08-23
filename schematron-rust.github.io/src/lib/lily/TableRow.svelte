<script lang="ts">
  // Lily Design System — TableRow. Local stand-in; see ./README.md.
  import type { Snippet } from 'svelte';

  let {
    class: className = '',
    children,
    ...restProps
  }: {
    class?: string;
    /** TableTH or TableTD cells. */
    children: Snippet;
    [key: string]: unknown;
  } = $props();
</script>

<tr class={`table-row ${className}`} {...restProps}>
  {@render children()}
</tr>
