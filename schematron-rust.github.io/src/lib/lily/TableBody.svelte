<script lang="ts">
  // Lily Design System — TableBody. Local stand-in; see ./README.md.
  import type { Snippet } from 'svelte';

  let {
    class: className = '',
    children,
    ...restProps
  }: {
    class?: string;
    /** TableRow elements with data cells. */
    children: Snippet;
    [key: string]: unknown;
  } = $props();
</script>

<tbody class={`table-body ${className}`} {...restProps}>
  {@render children()}
</tbody>
