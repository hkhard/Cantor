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
  import { ask, confirmBox, primeKeyboard } from './prompts.svelte'
  import { sortable } from './reorder'

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

  // iOS home-screen style editing: a long press lifts a card and turns on
  // edit mode (cards wiggle, drags start at once); Done ends it.
  let editMode = $state(false)
  let scrollerEl: HTMLElement | undefined = $state()
  let tabsEl: HTMLElement | undefined = $state()

  async function reorderScores(ids: string[]) {
    // Search results are a subset: keep the others in their places.
    await api.setOrder('scores', ids)
    await onrefresh()
  }

  /** One-tap sort in edit mode; the result becomes the manual order. */
  async function sortBy(key: 'title' | 'composer') {
    const k = (s: ScoreEntry) =>
      key === 'title' ? s.title.toLowerCase() : `${(metadata[s.file]?.composer ?? '').toLowerCase() || '\uffff'} ${s.title.toLowerCase()}`
    const ids = [...scores].sort((a, b) => k(a).localeCompare(k(b), 'sv')).map((s) => s.file)
    await api.setOrder('scores', ids)
    await onrefresh()
  }

  async function reorderTabs(ids: string[]) {
    await api.setOrder('setlists', ids)
    await reload()
  }

  async function reorderItems(sl: Setlist, keys: string[]) {
    const old = [...sl.items]
    sl.items = keys.map((k) => old[Number(k)])
    await saveList(sl)
  }

  /** Drop a score on another one: a new list with both (iOS folder). */
  async function makeList(dragged: string, target: string) {
    const a = scores.find((x) => x.file === target)
    const b = scores.find((x) => x.file === dragged)
    if (!a || !b) return
    const name = (await ask(t('lib.setlistName'), t('lib.newListFrom', { a: a.title, b: b.title }))) ?? ''
    if (!name.trim()) return
    const id = await api.createSetlist(name.trim())
    await reload()
    const sl = setlists.find((x) => x.id === id)
    if (!sl) return
    sl.items.push({ kind: 'score', score: a.file, start_page: null }, { kind: 'score', score: b.file, start_page: null })
    await saveList(sl)
    notice = t('lib.listMade', { name: sl.name })
  }

  /** Drop a score on a setlist tab: add it to the end of that list. */
  async function dropOnTab(dragged: string, setlistId: string) {
    const sl = setlists.find((x) => x.id === setlistId)
    const s = scores.find((x) => x.file === dragged)
    if (!sl || !s) return
    sl.items.push({ kind: 'score', score: s.file, start_page: null })
    await saveList(sl)
    notice = t('lib.addedTo', { title: s.title, name: sl.name })
  }

  let pending: string[] = $state([])
  async function reload() {
    pending = await api.pendingBackups().catch(() => [])
    const d = await api.libraryData()
    metadata = d.metadata
    setlists = d.setlists
    if (tab && !setlists.some((s) => s.id === tab)) tab = ''
  }
  onMount(() => {
    reload()
    // Files dropped in through the Files app arrive while Cantor is in the
    // background: look again when it comes back.
    // DATA-02: sync with the chosen folder on launch and when coming back.
    const syncThenRefresh = async () => {
      const r = await api.syncNow().catch(() => null)
      if (r?.folder && r.changed) notice = t('sync.done', { sent: r.sent, received: r.received })
      await onrefresh()
      await reload()
    }
    syncThenRefresh()
    const onVis = () => document.visibilityState === 'visible' && syncThenRefresh()
    document.addEventListener('visibilitychange', onVis)
    return () => document.removeEventListener('visibilitychange', onVis)
  })
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

  async function restorePending(name: string, discard: boolean) {
    if (!discard && !(await confirmBox(t('backup.restoreConfirm')))) return
    try {
      const n = await api.restorePending(name, discard)
      notice = discard ? '' : t('backup.restored', { n })
    } catch (e) {
      notice = t('backup.failed', { e: String(e) })
    }
    await onrefresh()
    await reload()
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

<main class="library" bind:this={scrollerEl} class:editing={editMode}>
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
  {#each pending as name (name)}
    <div class="pending glass" role="alert">
      <span>{t('backup.waiting', { name })}</span>
      <button class="chip on" onclick={() => restorePending(name, false)}>{t('backup.restoreNow')}</button>
      <button class="chip" onclick={() => restorePending(name, true)}>{t('backup.discard')}</button>
    </div>
  {/each}

  <div class="tabs" bind:this={tabsEl}>
    <div
      class="tablist"
      role="tablist"
      aria-label={t('lib.setlists')}
      use:sortable={{ onreorder: reorderTabs, onlift: () => (editMode = true), immediate: () => editMode }}
    >
      <button role="tab" class="chip" class:on={tab === ''} aria-selected={tab === ''} onclick={() => (tab = '')}>{t('lib.all')}</button>
      {#each setlists as sl (sl.id)}
        <button
          role="tab"
          class="chip"
          class:on={tab === sl.id}
          aria-selected={tab === sl.id}
          data-sort-id={sl.id}
          data-drop={sl.id}
          onclick={() => (tab = sl.id)}>{sl.name}</button
        >
      {/each}
    </div>
    <button class="chip" onclick={newSetlist}>{t('lib.newSetlist')}</button>
    {#if editMode}
      <button class="chip on done" onclick={() => (editMode = false)}>{t('common.done')}</button>
    {/if}
  </div>
  {#if editMode && !current}
    <div class="sortbar" role="group" aria-label={t('lib.sortBy')}>
      <button class="chip" onclick={() => sortBy('title')}>{t('lib.sortTitle')}</button>
      <button class="chip" onclick={() => sortBy('composer')}>{t('lib.sortComposer')}</button>
    </div>
  {/if}
  {#if editMode}<p class="hint sort-hint">{current ? t('lib.sortHintList') : t('lib.sortHint')}</p>{/if}

  {#if !current}
    <div class="libbar">
      <input class="search" type="search" placeholder={t('lib.search')} bind:value={query} oninput={onSearch} aria-label={t('lib.search')} />
      <button class="chip on" onclick={() => fileInput?.click()} disabled={importing}>{importing ? t('lib.importing') : t('lib.import')}</button>
      <input
        bind:this={fileInput}
        type="file"
        multiple
        accept=".mxl,.musicxml,.xml,.pdf,.4sc,.4ss,.cantorset,.cantorbackup,application/pdf,application/vnd.recordare.musicxml,application/vnd.recordare.musicxml+xml"
        style="display:none"
        onchange={importPicked}
      />
      {#if notice}<span class="notice">{notice}</span>{/if}
    </div>
    <ul
      class="scores"
      use:sortable={{
        onreorder: reorderScores,
        onmerge: makeList,
        onmergegesture: primeKeyboard,
        ondrop: dropOnTab,
        dropRoot: () => tabsEl,
        onlift: () => (editMode = true),
        immediate: () => editMode,
        disabled: () => hits !== null,
        scroller: () => scrollerEl,
      }}
    >
      {#each shown as s (s.file)}
        <li data-sort-id={s.file}>
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
          <button class="side info" data-nodrag onclick={() => (editing = s)} aria-label="{t('lib.edit')} {s.title}">{t('lib.edit')}</button>
          {#if setlists.length}
            <button class="side add" data-nodrag onclick={() => (addTo = addTo === s ? null : s)} aria-label="{t('set.add')}: {s.title}">＋</button>
          {/if}
          {#if s.user}
            <button class="remove" data-nodrag onclick={() => removeScore(s)} aria-label="{t('common.delete')} {s.title}">×</button>
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
    <ol
      class="scores setlist"
      use:sortable={{
        onreorder: (keys) => reorderItems(current!, keys),
        onlift: () => (editMode = true),
        immediate: () => editMode,
        scroller: () => scrollerEl,
      }}
    >
      {#each current.items as it, i (`${i}-${it.kind === 'score' ? it.score : it.title}`)}
        <li data-sort-id={String(i)}>
          {#if it.kind === 'score' && byId.get(it.score)}
            <button class="open" onclick={() => openItem(current!, i)}><span class="num">{i + 1}</span> {byId.get(it.score)!.title}</button>
          {:else}
            <span class="open missing"
              ><span class="num">{i + 1}</span>
              {it.kind === 'placeholder' ? it.title : it.score} <span class="badge">{t('set.missing')}</span></span
            >
          {/if}
          <button class="side up" data-nodrag onclick={() => move(current!, i, -1)} disabled={i === 0} aria-label={t('set.up')}>↑</button>
          <button class="side down" data-nodrag onclick={() => move(current!, i, 1)} disabled={i === current!.items.length - 1} aria-label={t('set.down')}>↓</button>
          <button
            data-nodrag
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
{#if aboutOpen}<About
    onclose={() => (aboutOpen = false)}
    onrestored={async () => {
      await onrefresh()
      await reload()
    }}
  />{/if}

<style>
  .pending {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    padding: 10px 14px;
    border-radius: 14px;
    margin-bottom: 10px;
  }
  .tablist {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .sortbar {
    display: flex;
    gap: 6px;
    margin-bottom: 8px;
  }
  .sort-hint {
    margin: -4px 0 10px;
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
