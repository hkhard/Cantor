<script lang="ts">
  // SET-03: render instructions text (see instructions.ts). Bar references
  // are buttons that call `onbar` with the 1-based bar number.
  import { parseInstructions, type Span } from './instructions'
  import { t } from './i18n.svelte'

  let { text, onbar }: { text: string; onbar?: (bar: number) => void } = $props()
  let paras = $derived(parseInstructions(text))
</script>

{#snippet spans(list: Span[])}
  {#each list as s}
    {#if s.kind === 'text'}{s.text}{:else if s.kind === 'br'}<br />{:else if s.kind === 'bold'}<strong
        >{@render spans(s.children)}</strong
      >{:else if s.kind === 'link'}<a href={s.href} target="_blank" rel="noopener noreferrer">{s.text}</a
      >{:else if s.kind === 'bar'}{#if onbar}<button class="barref" onclick={() => onbar?.(s.bar)} title={t('ins.goBar', { n: s.bar })}
          >{s.text}</button
        >{:else}{s.text}{/if}{/if}
  {/each}
{/snippet}

<div class="ins">
  {#each paras as p}
    <p>{@render spans(p)}</p>
  {/each}
</div>

<style>
  .ins p {
    margin: 0 0 0.6rem;
    line-height: 1.45;
    overflow-wrap: anywhere;
  }
  .ins a {
    color: var(--accent);
    text-decoration: underline;
  }
  .barref {
    display: inline;
    min-height: 0;
    padding: 0 0.3rem;
    margin: 0 0.05rem;
    font: inherit;
    font-weight: 600;
    color: var(--accent-ink, #fff);
    background: var(--accent, #b83e26);
    border: 0;
    border-radius: 0.4rem;
    cursor: pointer;
  }
</style>
