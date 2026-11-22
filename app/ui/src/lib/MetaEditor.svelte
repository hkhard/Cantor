<script lang="ts">
  // LIB-03: edit a score's metadata (title, composer, arranger, key, tempo,
  // genre, tags, rating, difficulty). Saved to the library store.
  import { api, emptyMetadata, type Metadata } from './api'
  import { t } from './i18n.svelte'

  interface Props {
    id: string
    title: string
    initial: Metadata | undefined
    onclose: (saved: boolean) => void
  }
  let { id, title, initial, onclose }: Props = $props()

  // Editing a copy taken when the sheet opens (the parent remounts it per score).
  const start = (): Metadata => {
    const c = structuredClone($state.snapshot(initial) ?? emptyMetadata(title)) as Metadata
    if (!c.title) c.title = title
    return c
  }
  let m: Metadata = $state(start())
  let tags = $state(m.tags.join(', '))
  let saving = $state(false)

  async function save() {
    saving = true
    m.tags = tags
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean)
    await api.setMetadata(id, $state.snapshot(m) as Metadata)
    saving = false
    onclose(true)
  }

  function stars(n: number | null, set: (v: number | null) => void) {
    return [1, 2, 3, 4, 5].map((i) => ({ i, on: (n ?? 0) >= i, set: () => set(n === i ? null : i) }))
  }
</script>

<div class="sheetbg" role="presentation" onclick={() => onclose(false)}></div>
<div class="metasheet glass" role="dialog" aria-label={t('meta.title')}>
  <h3>{t('meta.title')}</h3>
  <label>{t('meta.name')}<input bind:value={m.title} /></label>
  <label>{t('meta.composer')}<input bind:value={m.composer} /></label>
  <label>{t('meta.arranger')}<input bind:value={m.arranger} /></label>
  <div class="row">
    <label>{t('meta.key')}<input bind:value={m.key} placeholder="F-dur" /></label>
    <label
      >{t('meta.tempo')}<input
        type="number"
        min="20"
        max="300"
        value={m.tempo ?? ''}
        oninput={(e) => {
          const v = (e.target as HTMLInputElement).value
          m.tempo = v ? Number(v) : null
        }}
      /></label
    >
  </div>
  <label>{t('meta.genre')}<input bind:value={m.genre} /></label>
  <label>{t('meta.tags')}<input bind:value={tags} placeholder={t('meta.tagsHint')} /></label>
  <div class="row">
    <span class="stars" role="group" aria-label={t('meta.rating')}
      >{t('meta.rating')}
      {#each stars(m.rating, (v) => (m.rating = v)) as s}
        <button class:on={s.on} onclick={s.set} aria-label="{s.i}">★</button>
      {/each}
    </span>
    <span class="stars" role="group" aria-label={t('meta.difficulty')}
      >{t('meta.difficulty')}
      {#each stars(m.difficulty, (v) => (m.difficulty = v)) as s}
        <button class:on={s.on} onclick={s.set} aria-label="{s.i}">●</button>
      {/each}
    </span>
  </div>
  <div class="actions">
    <button onclick={() => onclose(false)}>{t('common.cancel')}</button>
    <button class="primary" onclick={save} disabled={saving}>{t('common.save')}</button>
  </div>
</div>

<style>
  .sheetbg {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.25);
    z-index: 40;
  }
  .metasheet {
    position: fixed;
    z-index: 41;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: min(520px, 92vw);
    max-height: 88vh;
    overflow: auto;
    padding: 18px 20px;
    border-radius: 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  h3 {
    margin: 0 0 4px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 0.8125rem;
    flex: 1;
  }
  input {
    font-size: 1rem;
    padding: 7px 9px;
    border-radius: 9px;
    border: 1px solid rgba(0, 0, 0, 0.15);
    background: rgba(255, 255, 255, 0.85);
  }
  .row {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
  }
  .stars {
    display: flex;
    align-items: center;
    gap: 2px;
    font-size: 0.8125rem;
  }
  .stars button {
    background: none;
    border: none;
    font-size: 1.25rem;
    color: #8a847c;
    padding: 4px 6px;
    min-width: 32px;
    min-height: 32px;
  }
  .stars button.on {
    color: var(--accent, #b8562b);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 6px;
  }
</style>
