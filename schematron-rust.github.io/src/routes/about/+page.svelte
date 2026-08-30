<script lang="ts">
  import SectionHeading from '$lib/lily/SectionHeading.svelte';
  import SummaryList from 'lily-design-system-svelte-headless/components/SummaryList/SummaryList.svelte';
  import SummaryListItem from 'lily-design-system-svelte-headless/components/SummaryListItem/SummaryListItem.svelte';
  import CodeBlock from '$lib/lily/CodeBlock.svelte';
  import InformationCallout from 'lily-design-system-svelte-headless/components/InformationCallout/InformationCallout.svelte';
  import Separator from 'lily-design-system-svelte-headless/components/Separator/Separator.svelte';
  import { VERSION, MSRV, REPO, CRATES_IO, DOCS_RS, LICENSE } from '$lib/site';
</script>

<svelte:head>
  <title>About — schematron</title>
  <meta
    name="description"
    content="About the schematron crate: what it is, who maintains it, how it is licensed, and how this site is built."
  />
</svelte:head>

<div class="page-header">
  <h1>About</h1>
  <p>
    <code>schematron</code> is a pure Rust implementation of ISO/IEC 19757-3,
    rule-based XML validation.
  </p>
</div>

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="The crate" heading="Facts" level={2} />

  <SummaryList label="Crate facts">
    <SummaryListItem term="Name"><code>schematron</code></SummaryListItem>
    <SummaryListItem term="Version">{VERSION}</SummaryListItem>
    <SummaryListItem term="MSRV">{MSRV} — policy: current stable minus two</SummaryListItem>
    <SummaryListItem term="Standard">ISO/IEC 19757-3 (Schematron)</SummaryListItem>
    <SummaryListItem term="Licence">{LICENSE}</SummaryListItem>
    <SummaryListItem term="Author">Joel Parker Henderson</SummaryListItem>
    <SummaryListItem term="Repository"><a href={REPO}>{REPO}</a></SummaryListItem>
    <SummaryListItem term="Package"><a href={CRATES_IO}>crates.io/crates/schematron</a></SummaryListItem>
    <SummaryListItem term="API docs"><a href={DOCS_RS}>docs.rs/schematron</a></SummaryListItem>
  </SummaryList>

  <p>
    The repository is mirrored to Codeberg and GitLab; GitHub is the primary
    remote.
  </p>
</section>

<Separator label="Section break" />

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="Contributing" heading="How to work on it" level={2} />

  <p>
    <code>AGENTS.md</code> in the repository is the entry point for contributors
    and coding agents alike: gates, non-negotiables, and where each fact lives.
    <code>spec/</code> is normative — a behaviour change is a specification
    change first.
  </p>

  <CodeBlock label="The gates a change must pass">
    <pre><code>{`cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --no-deps --all-features
cargo +${MSRV} test --all-features`}</code></pre>
  </CodeBlock>

  <p>
    Slower, and run when relevant: <code>cargo bench</code>, and
    <code>cargo +nightly fuzz run fuzz_validate</code> (needs
    <code>cargo-fuzz</code>).
  </p>
  <p>
    Adding a conformance case means adding a directory under
    <code>tests/corpus/</code> holding <code>schema.sch</code>,
    <code>input.xml</code>, and <code>expected.txt</code>. There is no Rust to
    change.
  </p>
</section>

<Separator label="Section break" />

<section class="section prose">
  <SectionHeading class="section-heading-start" eyebrow="This site" heading="How the site is built" level={2} />

  <p>
    This site is a <a href="https://kit.svelte.dev/">SvelteKit</a> project using
    <code>@sveltejs/adapter-static</code>, prerendered to plain HTML and
    deployed to GitHub Pages by GitHub Actions on every push to
    <code>main</code>.
  </p>

  <InformationCallout label="Design system">
    <p>
      The components come from the
      <a href="https://lilydesignsystem.com/">Lily Design System</a> — headless
      Svelte components that render semantic HTML and correct ARIA, carrying one
      stable class hook each and shipping no CSS at all. Every visual decision
      on this site therefore lives in a single stylesheet,
      <code>static/assets/style.css</code>, which targets those hooks. Replace
      that one file and the markup is unchanged.
    </p>
  </InformationCallout>

  <CodeBlock label="Run the site locally">
    <pre><code>{`git clone https://github.com/schematron-rust/schematron-rust.github.io
cd schematron-rust.github.io
pnpm install
pnpm dev`}</code></pre>
  </CodeBlock>

  <p style="margin-top: 2rem;">
    <a class="button button-primary" href={REPO}>The crate on GitHub</a>
    <a class="button button-secondary" href="/spec/">The specification</a>
  </p>
</section>
