<script lang="ts">
  // PLAY-09: choose the instrument of each voice, and a "voice set": the
  // instruments for a whole choir at once (built-in sets per choir type, or
  // your own, saved from the current mixer).
  import { onMount } from 'svelte'
  import { api, type ChannelInfo, type VoiceSet } from './api'
  import { i18n, t } from './i18n.svelte'
  import { ask } from './prompts.svelte'

  interface Props {
    file: string
    channels: ChannelInfo[]
    /** Choir type the score was opened with ('auto' | 'mixed' | 'male' | 'female'). */
    choir: string
    /** Re-open the score so a new voice set takes effect. */
    onreload: () => void
  }
  let { file, channels = $bindable(), choir, onreload }: Props = $props()

  let gm: [number, string][] = $state([])
  let sets: VoiceSet[] = $state([])
  let defaults: Record<string, string> = $state({})
  let current = $state('')

  // GM families of 8, named, for a grouped picker.
  const FAMILIES = [
    'Piano', 'Chromatic percussion', 'Organ', 'Guitar', 'Bass', 'Strings', 'Ensemble', 'Brass',
    'Reed', 'Pipe', 'Synth lead', 'Synth pad', 'Synth effects', 'Ethnic', 'Percussive', 'Sound effects',
  ]

  const setName = (v: VoiceSet) => (i18n.lang === 'sv' ? v.name_sv : v.name_en)
  const choirKey = $derived(choir === 'male' || choir === 'female' ? choir : 'mixed')
  const fits = (v: VoiceSet) => !v.choir || v.choir === choirKey

  async function load() {
    const [g, v, s] = await Promise.all([api.instruments(), api.voiceSets(), api.scoreSettings(file)])
    gm = g
    sets = v.sets
    defaults = v.defaults
    current = s.voice_set ?? defaults[choirKey] ?? 'classic'
  }
  onMount(load)

  async function setInstrument(i: number, e: Event) {
    const p = Number((e.target as HTMLSelectElement).value)
    await api.setInstrument(file, i, p)
    channels[i] = { ...channels[i], program: p, instrument: gm[p]?.[1] ?? channels[i].instrument }
  }

  async function useSet(e: Event) {
    current = (e.target as HTMLSelectElement).value
    await api.applyVoiceSet(current, file)
    onreload()
  }

  async function makeDefault() {
    await api.applyVoiceSet(current, null, choirKey)
    defaults = { ...defaults, [choirKey]: current }
  }

  /** Save what the mixer has now as an own set: the first line of each role. */
  async function saveAsSet() {
    const name = (await ask(t('vs.name'), t('vs.nameSuggest')))?.trim()
    if (!name) return
    const roles: Record<string, number[]> = {}
    for (const c of channels) {
      const r = c.role.toLowerCase()
      if (r === 'track') continue
      ;(roles[r] ??= []).push(c.program)
    }
    const id = await api.saveVoiceSet({ id: '', name_sv: name, name_en: name, choir: choirKey, roles })
    await load()
    current = id
    await api.applyVoiceSet(id, file)
  }

  async function removeSet() {
    await api.removeVoiceSet(current)
    await load()
    await api.applyVoiceSet(current, file)
    onreload()
  }
</script>

<div class="vsbar">
  <label
    >{t('vs.set')}
    <select value={current} onchange={useSet}>
      {#each sets.filter(fits) as v (v.id)}
        <option value={v.id}>{setName(v)}{defaults[choirKey] === v.id ? ` (${t('vs.default')})` : ''}</option>
      {/each}
    </select>
  </label>
  <button class="chip" onclick={makeDefault} disabled={defaults[choirKey] === current}
    >{t('vs.makeDefault', { choir: t(`vs.choir.${choirKey}`) })}</button
  >
  <button class="chip" onclick={saveAsSet}>{t('vs.save')}</button>
  {#if current.startsWith('own-')}
    <button class="chip" onclick={removeSet}>{t('vs.remove')}</button>
  {/if}
</div>

{#each channels as c, i (i)}
  {#if c.role !== 'Track'}
    <label class="instsel"
      ><span>{c.label}</span>
      <select value={String(c.program)} onchange={(e) => setInstrument(i, e)} aria-label={t('vs.instrumentFor', { label: c.label })}>
        {#each FAMILIES as fam, f}
          <optgroup label={fam}>
            {#each gm.slice(f * 8, f * 8 + 8) as [p, name] (p)}
              <option value={String(p)}>{name}</option>
            {/each}
          </optgroup>
        {/each}
      </select>
    </label>
  {/if}
{/each}

<style>
  .vsbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin: 6px 0 8px;
  }
  .vsbar label,
  .instsel {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.85rem;
  }
  .instsel {
    justify-content: space-between;
    padding: 4px 0;
  }
  .instsel span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  select {
    font: inherit;
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 999px;
    padding: 5px 10px;
    max-width: 60%;
  }
  .vsbar select {
    max-width: 100%;
  }
</style>
