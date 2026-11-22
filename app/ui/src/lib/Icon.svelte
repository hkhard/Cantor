<script lang="ts">
  // Minimal line-icon set (SF Symbols–like, 24px grid, currentColor).
  // Replaces unicode glyphs that iOS renders as emoji.
  type Name = 'play' | 'pause' | 'start' | 'back' | 'sliders' | 'appearance' | 'loop' | 'metronome'
  let { name, size = 22 }: { name: Name; size?: number } = $props()
</script>

<svg class="ic" width={size} height={size} viewBox="0 0 24 24" aria-hidden="true">
  {#if name === 'play'}
    <path d="M8 5.5v13a1 1 0 0 0 1.5.86l10.5-6.5a1 1 0 0 0 0-1.72L9.5 4.64A1 1 0 0 0 8 5.5z" fill="currentColor" />
  {:else if name === 'pause'}
    <rect x="6.5" y="5" width="4" height="14" rx="1.2" fill="currentColor" />
    <rect x="13.5" y="5" width="4" height="14" rx="1.2" fill="currentColor" />
  {:else if name === 'start'}
    <rect x="5" y="5.5" width="2.4" height="13" rx="1" fill="currentColor" />
    <path d="M19 6.4v11.2a.9.9 0 0 1-1.4.75L9.4 12.75a.9.9 0 0 1 0-1.5l8.2-5.6A.9.9 0 0 1 19 6.4z" fill="currentColor" />
  {:else if name === 'back'}
    <path d="M15 5l-7 7 7 7" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" />
  {:else if name === 'sliders'}
    <path d="M6 4v16M12 4v16M18 4v16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" opacity="0.45" />
    <g fill="currentColor"><rect x="3.5" y="13" width="5" height="3" rx="1.5" /><rect x="9.5" y="7" width="5" height="3" rx="1.5" /><rect x="15.5" y="11" width="5" height="3" rx="1.5" /></g>
  {:else if name === 'loop'}
    <path d="M17 7H8a4 4 0 0 0 0 8h1M7 17h9a4 4 0 0 0 0-8h-1" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
    <path d="M15 4.5 17.5 7 15 9.5M9 14.5 6.5 17 9 19.5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
  {:else if name === 'metronome'}
    <path d="M9 3.5h6l3.5 16.5h-13z" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round" />
    <path d="M12 16 17 6.5" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
  {:else if name === 'appearance'}
    <circle cx="12" cy="12" r="8" fill="none" stroke="currentColor" stroke-width="2" />
    <path d="M12 4a8 8 0 0 1 0 16z" fill="currentColor" />
  {/if}
</svg>

<style>
  .ic {
    display: block;
    flex: none;
  }
</style>
