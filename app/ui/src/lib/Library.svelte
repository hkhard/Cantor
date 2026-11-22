<script lang="ts">
  // Library screen: all scores with search (LIB-04), metadata (LIB-03),
  // setlists (SET-01, incl. ones imported from forScore, LIB-08), import and
  // remove (LIB-01/02), About (NFR-09).
  import { onMount } from 'svelte'
  import { api, type Metadata, type ScoreEntry, type Setlist } from './api'
  import Appearance from './Appearance.svelte'
  import MetaEditor from './MetaEditor.svelte'
  import About from './About.svelte'
  import { t } from './i18n.svelte'
  import { ask, confirmBox } from './prompts.svelte'

  interface Props {
    scores: ScoreEntry[]
    notice: string
    error: string
    loading: string
    /** Open a score; with a setlist, its position for next/previous. */
    onopen: (s: ScoreEntry, set?: { list: Setlist; index: number }) => void
    onrefresh: () => Promise<void>
  }
  let { scores, notice = $bindable(), error, loading, onopen, onrefresh }: Props = $props()

  let metadata: Record<string, Metadata> = $state({})
  let setlists: Setlist[] = $state([])
  /** '' = all scores, else a setlist id. */
  let tab = $state(localStorage.getItem('cantor.tab') ?? '')
  let query = $state('')
  let hits: Set<string> | null = $state(null)
  let editing: ScoreEntry | null = $state(null)
  let addTo: ScoreEntry | null = $state(null)
  let aboutOpen = $state(false)
  let importing = $state(false)
  let fileInput: HTMLInputElement | undefined = $state()

  async function reload() {
    const d = await api.libraryData()
    metadata = d.metadata
    setlists = d.setlists
    if (tab && !setlists.some((s) => s.id === tab)) tab = ''
  }
  onMount(reload)
  // Imports (also from the share sheet) can add setlists.
  $effect(() => {
    void scores
    reload()
  })
  $effect(() => {
    localStorage.setItem('cantor.tab', tab)
  })

  let searchTimer: number | undefined
  function onSearch() {
    clearTimeout(searchTimer)
    const q = query.trim()
    if (!q) {
      hits = null
      return
    }
    searchTimer = setTimeout(async () => {
      hits = new Set(await api.search(q))
    }, 120) as unknown as number
  }

  let shown = $derived(hits ? scores.filter((s) => hits!.has(s.file)) : scores)
  let current = $derived(setlists.find((s) => s.id === tab) ?? null)
  let byId = $derived(new Map(scores.map((s) => [s.file, s])))

  function sub(s: ScoreEntry): string {
    const m = metadata[s.file]
    return [m?.composer, m?.arranger && `arr. ${m.arranger}`, m?.tags?.length ? m.tags.join(', ') : '']
      .filter(Boolean)
      .join(' · ')
  }

  async function importPicked(e: Event) {
    const input = e.target as HTMLInputElement
    const files = [...(input.files ?? [])]
    input.value = ''
    if (!files.length) return
    importing = true
    notice = ''
    const added: string[] = []
    const failed: string[] = []
    for (const f of files) {
      try {
        added.push(...(await api.importFile(f)))
      } catch (err) {
        failed.push(String(err))
      }
    }
    importing = false
    await onrefresh()
    notice = [added.length ? t('lib.imported', { n: added.length }) : '', ...failed].filter(Boolean).join(' · ')
  }

  async function removeScore(s: ScoreEntry) {
    if (!(await confirmBox(t('lib.removeConfirm', { title: s.title })))) return
    await api.deleteScore(s.file)
    await onrefresh()
  }

  async function newSetlist() {
    const name = await ask(t('lib.setlistName'))
    if (!name) return
    tab = await api.createSetlist(name)
    await reload()
  }

  async function saveList(sl: Setlist) {
    await api.saveSetlist($state.snapshot(sl) as Setlist)
    await reload()
  }

  async function addToList(sl: Setlist, s: ScoreEntry) {
    sl.items.push({ kind: 'score', score: s.file, start_page: null })
    addTo = null
    await saveList(sl)
  }

  function move(sl: Setlist, i: number, d: number) {
    const j = i + d
    if (j < 0 || j >= sl.items.length) return
    ;[sl.items[i], sl.items[j]] = [sl.items[j], sl.items[i]]
    saveList(sl)
  }

  async function deleteList(sl: Setlist) {
    if (!(await confirmBox(t('set.deleteConfirm', { name: sl.name })))) return
    await api.deleteSetlist(sl.id)
    tab = ''
    await reload()
  }

  async function share(kind: 'setlist' | '4ss', id: string) {
    try {
      notice = t('share.done', { name: await api.exportItem(kind, id) })
    } catch (e) {
      notice = t('share.failed', { e: String(e) })
    }
  }

  function openItem(sl: Setlist, i: number) {
    const it = sl.items[i]
    if (it.kind !== 'score') return
    const s = byId.get(it.score)
    if (s) onopen(s, { list: sl, index: i })
  }
</script>

<main class="library">
  <header>
    <img class="mark" src="/cantor-mark.svg" alt="" width="64" height="64" />
    <div class="titles">
      <h1>Cantor</h1>
      <p class="sub">{t('lib.subtitle')}</p>
    </div>
    <button class="icon" onclick={() => (aboutOpen = true)} aria-label={t('lib.about')}>ⓘ</button>
    <Appearance />
  </header>
  {#if error}<p class="error">{error}</p>{/if}
  {#if loading}<p class="loading">{loading}</p>{/if}

  <div class="tabs">
    <div class="tablist" role="tablist" aria-label={t('lib.setlists')}>
    <button role="tab" class="chip" class:on={tab === ''} aria-selected={tab === ''} onclick={() => (tab = '')}>{t('lib.all')}</button>
    {#each setlists as sl (sl.id)}
      <button role="tab" class="chip" class:on={tab === sl.id} aria-selected={tab === sl.id} onclick={() => (tab = sl.id)}
        >{sl.name}</button
      >
    {/each}
    </div>
    <button class="chip" onclick={newSetlist}>{t('lib.newSetlist')}</button>
  </div>

  {#if !current}
    <div class="libbar">
      <input class="search" type="search" placeholder={t('lib.search')} bind:value={query} oninput={onSearch} aria-label={t('lib.search')} />
      <button class="chip on" onclick={() => fileInput?.click()} disabled={importing}>{importing ? t('lib.importing') : t('lib.import')}</button>
      <input
        bind:this={fileInput}
        type="file"
        multiple
        accept=".mxl,.musicxml,.xml,.pdf,.4sc,.4ss,.cantorset,application/pdf,application/vnd.recordare.musicxml,application/vnd.recordare.musicxml+xml"
        style="display:none"
        onchange={importPicked}
      />
      {#if notice}<span class="notice">{notice}</span>{/if}
    </div>
    <ul class="scores">
      {#each shown as s (s.file)}
        <li>
          <button class="open" onclick={() => onopen(s)}>
            <span class="t">
              <span>{s.title}</span>
              {#if sub(s)}<span class="meta">{sub(s)}</span>{/if}
            </span>
            <span class="badges">
              {#if s.playable}<span class="badge">{t('lib.plays')}</span>{/if}
              {#if s.pdf}<span class="badge">PDF</span>{/if}
              {#if s.user}<span class="badge mine">{t('lib.mine')}</span>{/if}
            </span>
          </button>
          <button class="side info" onclick={() => (editing = s)} aria-label="{t('lib.edit')} {s.title}">{t('lib.edit')}</button>
          {#if setlists.length}
            <button class="side add" onclick={() => (addTo = addTo === s ? null : s)} aria-label="{t('set.add')}: {s.title}">＋</button>
          {/if}
          {#if s.user}
            <button class="remove" onclick={() => removeScore(s)} aria-label="{t('common.delete')} {s.title}">×</button>
          {/if}
          {#if addTo === s}
            <span class="addmenu glass">
              {#each setlists as sl (sl.id)}
                <button onclick={() => addToList(sl, s)}>{sl.name}</button>
              {/each}
            </span>
          {/if}
        </li>
      {:else}
        <li class="empty">{t('lib.noHits')}</li>
      {/each}
    </ul>
    <p class="note">{t('lib.note')}</p>
  {:else}
    <div class="libbar">
      <h2 class="setname">{current.name}</h2>
      {#if notice}<span class="notice">{notice}</span>{/if}
      {#if current.items.length}
        <button class="chip on" onclick={() => openItem(current!, current!.items.findIndex((i) => i.kind === 'score'))}>{t('set.playlist')}</button>
      {/if}
      <button class="chip" onclick={() => share('setlist', current!.id)}>{t('share.title')}: Cantor</button>
      <button class="chip" onclick={() => share('4ss', current!.id)}>forScore</button>
      <button class="chip" onclick={() => deleteList(current!)}>{t('common.delete')}</button>
    </div>
    <ol class="scores setlist">
      {#each current.items as it, i (i)}
        <li>
          {#if it.kind === 'score' && byId.get(it.score)}
            <button class="open" onclick={() => openItem(current!, i)}><span class="num">{i + 1}</span> {byId.get(it.score)!.title}</button>
          {:else}
            <span class="open missing"
              ><span class="num">{i + 1}</span>
              {it.kind === 'placeholder' ? it.title : it.score} <span class="badge">{t('set.missing')}</span></span
            >
          {/if}
          <button class="side up" onclick={() => move(current!, i, -1)} disabled={i === 0} aria-label={t('set.up')}>↑</button>
          <button class="side down" onclick={() => move(current!, i, 1)} disabled={i === current!.items.length - 1} aria-label={t('set.down')}>↓</button>
          <button
            class="remove"
            onclick={() => {
              current!.items.splice(i, 1)
              saveList(current!)
            }}
            aria-label={t('set.remove')}>×</button
          >
        </li>
      {:else}
        <li class="empty">{t('set.empty')}</li>
      {/each}
    </ol>
  {/if}
</main>

{#if editing}
  <MetaEditor
    id={editing.file}
    title={editing.title}
    initial={metadata[editing.file]}
    onclose={async (saved) => {
      editing = null
      if (saved) {
        await reload()
        await onrefresh()
      }
    }}
  />
{/if}
{#if aboutOpen}<About onclose={() => (aboutOpen = false)} />{/if}

<style>
  .tablist {
    display: contents;
  }
  .tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 4px 0 10px;
  }
  .search {
    flex: 1;
    min-width: 200px;
    font-size: 1rem;
    padding: 8px 12px;
    border-radius: 12px;
    border: 1px solid rgba(0, 0, 0, 0.12);
    background: var(--glass-bg-strong, rgba(255, 255, 255, 0.8));
    color: inherit;
  }
  .scores li {
    position: relative;
  }
  .t {
    padding-right: 28px;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    min-width: 0;
  }
  .meta {
    font-size: 0.75rem;
    opacity: 0.65;
  }
  .scores :global(.side) {
    position: absolute;
    bottom: 8px;
    width: auto;
    min-width: 34px;
    height: 28px;
    min-height: 0;
    padding: 0 10px;
    font-size: 0.8125rem;
    font-family: -apple-system, system-ui, sans-serif;
    border-radius: 14px;
  }
  .scores :global(.side.info) {
    right: 8px;
  }
  .scores :global(.side.add) {
    right: 64px;
  }
  .scores :global(.side.up) {
    right: 84px;
    bottom: auto;
    top: 50%;
    transform: translateY(-50%);
  }
  .scores :global(.side.down) {
    right: 42px;
    bottom: auto;
    top: 50%;
    transform: translateY(-50%);
  }
  .setlist :global(.remove) {
    top: 50% !important;
    transform: translateY(-50%);
  }
  .setlist li > :global(.open) {
    min-height: 56px;
    padding-right: 130px;
    flex-direction: row !important;
    align-items: center !important;
    justify-content: flex-start !important;
  }
  .setlist {
    grid-template-columns: 1fr !important;
    max-width: 760px;
  }
  .addmenu :global(button) {
    min-height: 40px;
    font-size: 0.9375rem;
    padding: 8px 14px;
  }
  .addmenu {
    position: absolute;
    right: 8px;
    top: calc(100% - 6px);
    z-index: 10;
    display: flex;
    flex-direction: column;
    padding: 6px;
    border-radius: 12px;
    gap: 4px;
  }
  .setname {
    flex: 1;
    margin: 0;
    font-size: 1.25rem;
  }
  .num {
    display: inline-block;
    min-width: 1.8em;
    opacity: 0.55;
  }
  .missing {
    opacity: 0.6;
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 56px;
    padding: 12px 18px;
    border-radius: var(--radius-m);
    border: 1px dashed var(--glass-border);
  }
  .empty {
    opacity: 0.65;
    padding: 12px;
  }
</style>
