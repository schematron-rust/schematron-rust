<script lang="ts">
  // Lily Design System — CodeBlock
  //
  // Local stand-in; see ./README.md. A labelled wrapper around code content,
  // carrying the `code-block` hook. The consumer supplies the <pre><code>.
  //
  // Props:
  //   class — string, optional. CSS class name appended to the base hook.
  //   label — string, optional. Accessible label via aria-label.
  //   children — Snippet, required. The code content.

  import type { Snippet } from 'svelte';

  let {
    class: className = '',
    label = undefined,
    children,
    ...restProps
  }: {
    class?: string;
    label?: string;
    children: Snippet;
    [key: string]: unknown;
  } = $props();
</script>

<div class={`code-block ${className}`} aria-label={label} {...restProps}>
  {@render children()}
</div>
