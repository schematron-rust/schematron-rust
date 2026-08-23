<script lang="ts">
  // Lily Design System — TableTH. Local stand-in; see ./README.md.
  import type { Snippet } from 'svelte';

  let {
    class: className = '',
    colSpan = undefined,
    rowSpan = undefined,
    scope = 'col',
    children,
    ...restProps
  }: {
    class?: string;
    /** Number of columns this header cell spans. */
    colSpan?: number;
    /** Number of rows this header cell spans. */
    rowSpan?: number;
    /** Header scope. */
    scope?: 'col' | 'row' | 'colgroup' | 'rowgroup';
    /** Header cell content. */
    children?: Snippet;
    [key: string]: unknown;
  } = $props();
</script>

<th
  class={`table-th ${className}`}
  {scope}
  colspan={colSpan || undefined}
  rowspan={rowSpan || undefined}
  {...restProps}
>
  {@render children?.()}
</th>
