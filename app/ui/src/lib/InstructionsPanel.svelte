<script lang="ts">
  // SET-03/04/05 in the reader: the director's instructions for this piece
  // (and for the set and this item when opened from a setlist), who sings
  // what, the member's own part, and the practice tracks that came with a
  // shared set. Editable here; saved in the library store (synced, backed up).
  import { api, type InstructionsData, type PartHint } from './api'
  import Instructions from './Instructions.svelte'
  import { scoreFileUrl } from './pdf'
  import { t } from './i18n.svelte'

  interface Props {
    file: string
    /** Mixer channel labels, offered as parts. */
    parts: string[]
    setName?: string
    setText?: string
    note?: string
    data: InstructionsData
    onbar: (bar: number) => void
    onclose: () => void
    /** Saved or member changed: reload `data`. */
    onchanged: () => void
  }
  let { file, parts, setName = '', setText = '', note = '', data, onbar, onclose, onchanged }: Props = $props()

  let editing = $state(false)
  let text = $state('')
  let hints: PartHint[] = $state([])
  let saving = $state(false)
  let audioEl: HTMLAudioElement | undefined = $state()
  let playingPart = $state('')

  function edit() {
    text = data.text
    hints = data.parts.map((h) => ({ ...h }))
    if (!hints.length) hints.push({ who: '', part: parts[0] ?? '' })
    editing = true
  }
  async function save() {
    saving = true
    try {
      await api.setInstructions(file, { text, parts: $state.snapshot(hints) as PartHint[] })
      editing = false
      onchanged()
    } finally {
      saving = false
    }
  }
  async function pickMember(e: Event) {
    await api.setMember((e.target as HTMLSelectElement).value)
    onchanged()
  }
  let names = $derived([...new Set(data.parts.map((h) => h.who.trim()).filter(Boolean))])
  let myTrack = $derived(data.member_part ? data.practice.find((p) => p.part.toLowerCase() === data.member_part!.toLowerCase()) : undefined)

  function practise(p: { part: string; url: string }) {
    if (!audioEl) return
    if (playingPart === p.part && !audioEl.paused) {
      audioEl.pause()
      playingPart = ''
      return
    }
    audioEl.src = scoreFileUrl(p.url)
    playingPart = p.part
    audioEl.play().catch(() => (playingPart = ''))
  }
  function key(e: KeyboardEvent) {
    if (e.key === 'Escape' && !editing) onclose()
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section class="inspanel glass" aria-label={t('ins.title')} onkeydown={key}>
  <header>
    <h3>{t('ins.title')}</h3>
    {#if !editing}<button class="chip" onclick={edit}>{t('ins.edit')}</button>{/if}
    <button class="chip" onclick={onclose} aria-label={t('common.close')}>×</button>
  </header>

  {#if setText.trim()}
    <section class="set">
      <h4>{t('ins.forSet', { name: setName })}</h4>
      <Instructions text={setText} {onbar} />
    </section>
  {/if}
  {#if note.trim()}
    <div class="note"><h4>{t('ins.itemNote')}</h4><Instructions text={note} {onbar} /></div>
  {/if}

  {#if editing}
    <label class="col"
      >{t('ins.text')}
      <textarea bind:value={text} rows="7" placeholder={t('ins.placeholder')}></textarea>
    </label>
    <p class="hint">{t('ins.syntax')}</p>
    <fieldset>
      <legend>{t('ins.whoSings')}</legend>
      {#each hints as h, i}
        <div class="hintrow">
          <input bind:value={h.who} placeholder={t('ins.who')} aria-label={t('ins.who')} />
          {#if parts.length}
            <select bind:value={h.part} aria-label={t('ins.part')}>
              {#each parts as p}<option value={p}>{p}</option>{/each}
              {#if h.part && !parts.includes(h.part)}<option value={h.part}>{h.part}</option>{/if}
            </select>
          {:else}
            <input bind:value={h.part} placeholder={t('ins.part')} aria-label={t('ins.part')} />
          {/if}
          <button class="chip" onclick={() => hints.splice(i, 1)} aria-label={t('ins.removeHint', { who: h.who })}>×</button>
        </div>
      {/each}
      <button class="chip" onclick={() => hints.push({ who: '', part: parts[0] ?? '' })}>{t('ins.addHint')}</button>
    </fieldset>
    <div class="actions">
      <button onclick={() => (editing = false)}>{t('common.cancel')}</button>
      <button class="primary" onclick={save} disabled={saving}>{t('common.save')}</button>
    </div>
  {:else}
    {#if data.text.trim()}
      <Instructions text={data.text} {onbar} />
    {:else if !setText.trim() && !note.trim()}
      <p class="hint">{t('ins.empty')}</p>
    {/if}
    {#if data.parts.length}
      <section>
        <h4>{t('ins.whoSings')}</h4>
        <ul class="who">
          {#each data.parts as h}
            <li class:me={data.member && h.who.toLowerCase() === data.member.toLowerCase()}>{h.who}: <strong>{h.part}</strong></li>
          {/each}
        </ul>
        <label class="row"
          >{t('ins.iAm')}
          <select value={names.find((n) => n.toLowerCase() === data.member.toLowerCase()) ?? ''} onchange={pickMember}>
            <option value="">{t('ins.nobody')}</option>
            {#each names as n}<option value={n}>{n}</option>{/each}
          </select>
        </label>
        {#if data.member_part}<p class="mypart">{t('ins.yourPart', { part: data.member_part })}</p>{/if}
      </section>
    {/if}
  {/if}

  {#if data.practice.length && !editing}
    <section>
      <h4>{t('ins.practice')}</h4>
      {#if myTrack}
        <button class="primary practise" onclick={() => practise(myTrack!)}
          >{playingPart === myTrack.part ? t('ins.stop') : t('ins.practiseMine', { part: myTrack.part })}</button
        >
      {/if}
      <div class="tracks">
        {#each data.practice as p}
          <button class="chip" class:on={playingPart === p.part} aria-pressed={playingPart === p.part} onclick={() => practise(p)}>♪ {p.part}</button>
        {/each}
      </div>
      <audio bind:this={audioEl} preload="none" onended={() => (playingPart = '')}></audio>
    </section>
  {/if}
</section>

<style>
  .inspanel {
    position: absolute;
    top: 4rem;
    right: 0.75rem;
    z-index: 30;
    width: min(26rem, calc(100vw - 1.5rem));
    max-height: calc(100% - 9rem);
    overflow: auto;
    padding: 0.9rem 1rem;
    border-radius: 1.1rem;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    background: var(--glass-bg-strong, #fff);
  }
  header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  h3 {
    flex: 1;
    margin: 0;
    font-size: 1.125rem;
  }
  h4 {
    margin: 0.2rem 0 0.4rem;
    font-size: 0.95rem;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  textarea,
  input,
  select {
    font: inherit;
    font-size: 1rem;
    padding: 0.4rem 0.6rem;
    border-radius: 0.6rem;
    border: 1px solid var(--line, rgba(0, 0, 0, 0.2));
    background: var(--surface, #fff);
    color: var(--text, inherit);
  }
  fieldset {
    border: 1px solid var(--line, rgba(0, 0, 0, 0.15));
    border-radius: 0.8rem;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .hintrow {
    display: flex;
    gap: 0.4rem;
  }
  .hintrow input {
    flex: 1;
    min-width: 0;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }
  .who {
    margin: 0 0 0.4rem;
    padding-left: 1.1rem;
  }
  .who .me {
    background: var(--accent-soft);
    border-radius: 0.3rem;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .mypart {
    margin: 0.3rem 0 0;
    font-weight: 600;
  }
  .note {
    margin: 0;
  }
  .tracks {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    margin-top: 0.4rem;
  }
  .set {
    border-bottom: 1px solid var(--line, rgba(0, 0, 0, 0.12));
  }
  .hint {
    margin: 0;
    font-size: 0.85rem;
    color: var(--muted);
  }
</style>
