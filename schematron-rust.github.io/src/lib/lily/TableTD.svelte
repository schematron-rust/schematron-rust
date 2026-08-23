<script lang="ts">
  // Lily Design System — TableTD. Local stand-in; see ./README.md.
  import type { Snippet } from 'svelte';

  let {
    class: className = '',
    colSpan = undefined,
    rowSpan = undefined,
    children,
    ...restProps
  }: {
    class?: string;
    /** Number of columns this cell spans. */
    colSpan?: number;
    /** Number of rows this cell spans. */
    rowSpan?: number;
    /** Cell content. */
    children?: Snippet;
    [key: string]: unknown;
  } = $props();
</script>

<td
  class={`table-td ${className}`}
  colspan={colSpan || undefined}
  rowspan={rowSpan || undefined}
  {...restProps}
>
  {@render children?.()}
</td>
