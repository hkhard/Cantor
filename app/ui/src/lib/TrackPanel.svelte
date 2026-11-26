<script lang="ts">
  // PERF-04: a backing track (recording) for a score: attach, line up with
  // the score (offset) and page turns on the track timeline.
  import { api, type TrackInfo } from './api'
  import { t, pickerAccept } from './i18n.svelte'

  interface Props {
    file: string
    track: TrackInfo | null
    /** Current page shown (for "turn here"). */
    page: number
    /** Current playback position in score seconds. */
    position: number
    onchanged: () => void
  }
  let { file, track, page, position, onchanged }: Props = $props()

  let input: HTMLInputElement | undefined = $state()
  let busy = $state(false)
  let msg = $state('')
  // Editable copies of the saved settings (seeded when the panel opens).
  const seed = () => ({ offset: track?.offset_sec ?? 0, markers: [...(track?.markers ?? [])] })
  let offset = $state(seed().offset)
  let markers: [number, number][] = $state(seed().markers)

  async function pick(e: Event) {
    const el = e.target as HTMLInputElement
    const f = el.files?.[0]
    el.value = ''
    if (!f) return
    busy = true
    msg = ''
    try {
      await api.attachTrack(file, f)
      onchanged()
    } catch (err) {
      msg = String(err)
    }
    busy = false
  }

  async function save() {
    await api.setTrackInfo(file, offset, $state.snapshot(markers) as [number, number][])
  }

  function fmt(s: number) {
    const m = Math.floor(s / 60)
    return `${m}:${(s - m * 60).toFixed(1).padStart(4, '0')}`
  }

  async function addTurn() {
    // A turn to the next page at the current track time.
    const at = Math.max(0, position + offset)
    markers = [...markers.filter((m) => Math.abs(m[0] - at) > 0.05), [at, page + 1]].sort((a, b) => a[0] - b[0])
    await save()
  }

  async function remove() {
    await api.removeTrack(file)
    onchanged()
  }
</script>

<div class="col track">
  <span>{t('trk.title')}</span>
  {#if track}
    <label class="row"
      >{t('trk.offset')}
      <input
        type="number"
        step="0.1"
        value={offset}
        onchange={(e) => {
          offset = Number((e.target as HTMLInputElement).value) || 0
          save()
        }}
      />
    </label>
    <div class="row">
      <button onclick={addTurn}>{t('trk.addTurn', { page: page + 1 })}</button>
    </div>
    {#if markers.length}
      <ul class="markers">
        {#each markers as m, i (i)}
          <li>
            {t('trk.turnAt', { time: fmt(m[0]), page: m[1] })}
            <button
              class="x"
              aria-label={t('common.delete')}
              onclick={() => {
                markers = markers.filter((_, j) => j !== i)
                save()
              }}>×</button
            >
          </li>
        {/each}
      </ul>
    {/if}
    <div class="row">
      <button onclick={() => input?.click()} disabled={busy}>{t('trk.replace')}</button>
      <button onclick={remove}>{t('trk.remove')}</button>
    </div>
  {:else}
    <p class="hint">{t('trk.hint')}</p>
    <button onclick={() => input?.click()} disabled={busy}>{busy ? t('lib.importing') : t('trk.add')}</button>
  {/if}
  <input bind:this={input} type="file" accept={pickerAccept("audio/*,.m4a,.mp3,.wav,.flac,.aif,.aiff")} style="display:none" onchange={pick} aria-label={t('trk.add')} />
  {#if msg}<p class="hint" role="alert">{msg}</p>{/if}
</div>

<style>
  .track input[type='number'] {
    width: 6em;
    font-size: 1rem;
  }
  .markers {
    list-style: none;
    margin: 0;
    padding: 0;
    font-size: 0.875rem;
  }
  .markers li {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 6px;
  }
  .markers .x {
    min-width: 32px;
    padding: 2px 8px;
  }
</style>
