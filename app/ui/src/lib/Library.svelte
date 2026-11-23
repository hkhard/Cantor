<script lang="ts">
  // Library screen: all scores with search (LIB-04), metadata (LIB-03),
  // setlists (SET-01, incl. ones imported from forScore, LIB-08), import and
  // remove (LIB-01/02), About (NFR-09).
  import { onMount } from 'svelte'
  import { api, type HomeGroup, type Metadata, type ScoreEntry, type Setlist } from './api'
  import Appearance from './Appearance.svelte'
  import MetaEditor from './MetaEditor.svelte'
  import About from './About.svelte'
  import { t } from './i18n.svelte'
  import { ask, confirmBox, primeKeyboard } from './prompts.svelte'
  import { sortable } from './reorder'
  import Instructions from './Instructions.svelte'

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

  // First run (phase P): no scores of the user's own yet, only the bundled
  // demo or nothing. Explain how to add scores and offer Lacrymosa.
  let firstRun = $derived(!scores.some((s) => s.user))
  let demo = $derived(scores.find((s) => !s.user && /lacrymosa/i.test(s.title)))
  let metadata: Record<string, Metadata> = $state({})
  let setlists: Setlist[] = $state([])
  // LIB-04: the start view is a grouping (setlists by default, or genre,
  // tags, composer ...) and a selected tab ('' = all scores). Saved in the
  // library store, so it is restored on start and synced between devices.
  let group: HomeGroup = $state('setlists')
  /** '' = all scores; a setlist id (group 'setlists') or a group value. */
  let tab = $state(localStorage.getItem('cantor.tab') ?? '')
  let homeLoaded = false
  const GROUPS: HomeGroup[] = ['setlists', 'genre', 'tags', 'composer', 'arranger', 'rating', 'difficulty']
  /** Marker tab for scores without a value in the current grouping. */
  const NONE = '\u0000none'

  /** The values of `s` in grouping `g` (several for tags). */
  function valuesOf(s: ScoreEntry, g: HomeGroup): string[] {
    const m = metadata[s.file]
    switch (g) {
      case 'genre':
        return m?.genre?.trim() ? [m.genre.trim()] : []
      case 'tags':
        return (m?.tags ?? []).map((x) => x.trim()).filter(Boolean)
      case 'composer':
        return m?.composer?.trim() ? [m.composer.trim()] : []
      case 'arranger':
        return m?.arranger?.trim() ? [m.arranger.trim()] : []
      case 'rating':
        return m?.rating ? ['★'.repeat(m.rating)] : []
      case 'difficulty':
        return m?.difficulty ? ['●'.repeat(m.difficulty)] : []
      default:
        return []
    }
  }

  /** Tabs for the current non-setlist grouping: each value with its count. */
  let groupTabs = $derived.by(() => {
    if (group === 'setlists') return [] as { value: string; label: string; n: number }[]
    const counts = new Map<string, number>()
    let none = 0
    for (const s of scores) {
      const v = valuesOf(s, group)
      if (!v.length) none++
      for (const x of new Set(v)) counts.set(x, (counts.get(x) ?? 0) + 1)
    }
    const ordered = [...counts.entries()].sort((a, b) =>
      group === 'rating' || group === 'difficulty' ? b[0].length - a[0].length : a[0].localeCompare(b[0], 'sv'),
    )
    const tabs = ordered.map(([value, n]) => ({ value, label: value, n }))
    if (none && tabs.length) tabs.push({ value: NONE, label: t('lib.groupNone'), n: none })
    return tabs
  })

  function setGroup(g: HomeGroup) {
    group = g
    tab = ''
  }
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
  // LIB-05: suggestions to combine a MusicXML and a PDF of the same piece.
  let pairs: [string, string][] = $state([])
  async function combine(score: string, pdf: string, yes: boolean) {
    const name = byId.get(score)?.title ?? ''
    try {
      await api.combineScores(score, pdf, !yes)
      if (yes) notice = t('lib.combined', { title: name })
      await onrefresh()
    } catch (e) {
      notice = String(e)
    }
    pairs = await api.pairSuggestions().catch(() => [])
  }
  /** LIB-05: your own scores that are the other half of `s` (a PDF for
   * notes-only, notes for a PDF-only score), suggested pairs first. */
  function combinePartners(s: ScoreEntry): { file: string; title: string }[] {
    if (!s.user || s.playable === s.pdf) return []
    const other = scores.filter((x) => x.user && x.file !== s.file && x.playable === s.pdf && x.pdf === s.playable)
    const hinted = new Set(pairs.flat())
    return other
      .sort((a, b) => Number(hinted.has(b.file)) - Number(hinted.has(a.file)) || a.title.localeCompare(b.title))
      .map((x) => ({ file: x.file, title: x.title }))
  }
  // SET-03: editing the setlist's instructions; SET-04: a set just received.
  let setInsEdit: string | null = $state(null)
  let received = $state(false)
  async function saveSetInstructions(sl: Setlist) {
    sl.instructions = (setInsEdit ?? '').trim()
    setInsEdit = null
    await saveList(sl)
  }
  /** SET-05: edit the note of one setlist item. */
  async function editNote(sl: Setlist, i: number) {
    const it = sl.items[i]
    if (it.kind !== 'score') return
    const v = await ask(t('set.noteFor', { title: byId.get(it.score)?.title ?? it.score }), it.note ?? '')
    if (v === null) return
    it.note = v.trim()
    await saveList(sl)
  }
  async function reload() {
    // SET-04: a received .cantorset opens on its setlist with instructions.
    const got = await api.takeReceivedSet().catch(() => null)
    pending = await api.pendingBackups().catch(() => [])
    pairs = await api.pairSuggestions().catch(() => [])
    const d = await api.libraryData()
    metadata = d.metadata
    setlists = d.setlists
    if (!homeLoaded && d.home) {
      // The saved start view, once per launch.
      homeLoaded = true
      if (GROUPS.includes(d.home.group)) group = d.home.group
      tab = d.home.tab ?? ''
    }
    // After the saved start view, so a received set wins over it.
    if (got) {
      group = 'setlists'
      tab = got
      received = true
    }
    if (group === 'setlists' && tab && !setlists.some((s) => s.id === tab)) tab = ''
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
  let homeTimer: number | undefined
  $effect(() => {
    const v = { group, tab }
    localStorage.setItem('cantor.tab', tab)
    if (!homeLoaded) return
    clearTimeout(homeTimer)
    homeTimer = setTimeout(() => api.setHomeView(v).catch(() => {}), 300) as unknown as number
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

  let shown = $derived.by(() => {
    let v = hits ? scores.filter((s) => hits!.has(s.file)) : scores
    if (group !== 'setlists' && tab) {
      v = tab === NONE ? v.filter((s) => !valuesOf(s, group).length) : v.filter((s) => valuesOf(s, group).includes(tab))
    }
    return v
  })
  let current = $derived(group === 'setlists' ? (setlists.find((s) => s.id === tab) ?? null) : null)
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
        failed.push(t('lib.importFailed', { name: f.name, e: String(err) }))
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

  {#each pairs as [score, pdf] (score + pdf)}
    <div class="pending glass" role="alert">
      <span>{t('lib.combineAsk', { a: byId.get(score)?.title ?? score, b: byId.get(pdf)?.title ?? pdf })}</span>
      <button class="chip on" onclick={() => combine(score, pdf, true)}>{t('lib.combine')}</button>
      <button class="chip" onclick={() => combine(score, pdf, false)}>{t('lib.combineNo')}</button>
    </div>
  {/each}

  <div class="viewbar">
    <label class="viewsel"
      >{t('lib.show')}
      <select value={group} onchange={(e) => setGroup((e.target as HTMLSelectElement).value as HomeGroup)}>
        {#each GROUPS as g}
          <option value={g}>{t(`lib.group.${g}`)}</option>
        {/each}
      </select>
    </label>
  </div>
  <div class="tabs" bind:this={tabsEl}>
    {#if group !== 'setlists'}
      <div class="tablist" role="tablist" aria-label={t(`lib.group.${group}`)}>
        <button role="tab" class="chip" class:on={tab === ''} aria-selected={tab === ''} onclick={() => (tab = '')}>{t('lib.all')}</button>
        {#each groupTabs as g (g.value)}
          <button role="tab" class="chip" class:on={tab === g.value} aria-selected={tab === g.value} onclick={() => (tab = g.value)}
            >{g.label} <span class="count">{g.n}</span></button
          >
        {:else}
          <span class="hint">{t('lib.groupEmpty')}</span>
        {/each}
      </div>
    {:else}
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
    {/if}
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
        accept=".mxl,.musicxml,.xml,.pdf,.mscz,.4sc,.4ss,.cantorset,.cantorbackup,application/pdf,application/vnd.recordare.musicxml,application/vnd.recordare.musicxml+xml"
        style="display:none"
        onchange={importPicked}
      />
      {#if notice}<span class="notice">{notice}</span>{/if}
    </div>
    {#if firstRun}
      <section class="welcome glass" aria-labelledby="welcome-title">
        <h2 id="welcome-title">{t('first.title')}</h2>
        <p>{t('first.body')}</p>
        <ul class="how">
          <li><button class="linklike" onclick={() => fileInput?.click()}>{t('first.import')}</button></li>
          <li>{t('first.share')}</li>
          <li>{t('first.files')}</li>
        </ul>
        {#if demo}
          <button class="trycard" onclick={() => demo && onopen(demo)}>
            <b>{t('first.try')}</b>
            <span>{t('first.tryBody')}</span>
          </button>
        {/if}
      </section>
    {/if}
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
    {#if received}
      <p class="pending glass" role="status">{t('set.received', { name: current.name })}</p>
    {/if}
    <section class="setins glass" aria-label={t('ins.forSet', { name: current.name })}>
      {#if setInsEdit !== null}
        <label class="col"
          >{t('ins.forSet', { name: current.name })}
          <textarea bind:value={setInsEdit} rows="5" placeholder={t('ins.placeholder')}></textarea>
        </label>
        <p class="hint">{t('ins.syntax')}</p>
        <div class="row">
          <button class="chip" onclick={() => (setInsEdit = null)}>{t('common.cancel')}</button>
          <button class="chip on" onclick={() => saveSetInstructions(current!)}>{t('common.save')}</button>
        </div>
      {:else}
        <div class="row">
          <h3>{t('ins.title')}</h3>
          <button class="chip" onclick={() => (setInsEdit = current!.instructions ?? '')}>{t('ins.edit')}</button>
        </div>
        {#if current.instructions?.trim()}
          <Instructions text={current.instructions} />
        {:else}
          <p class="hint">{t('ins.emptySet')}</p>
        {/if}
      {/if}
    </section>
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
            <button class="open" onclick={() => openItem(current!, i)}
              ><span class="num">{i + 1}</span>
              <span class="t"
                ><span>{byId.get(it.score)!.title}</span>{#if it.note}<span class="meta itemnote">{it.note}</span>{/if}</span
              ></button
            >
            <button class="side notebtn" data-nodrag onclick={() => editNote(current!, i)} aria-label={t('set.noteFor', { title: byId.get(it.score)!.title })}
              >✎</button
            >
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
    pdf={editing.pdf}
    initial={metadata[editing.file]}
    partners={combinePartners(editing)}
    oncombine={async (other) => {
      const e = editing!
      const [score, pdf] = e.pdf ? [other, e.file] : [e.file, other]
      await api.combineScores(score, pdf, false)
      notice = t('lib.combined', { title: byId.get(score)?.title ?? '' })
      editing = null
      await onrefresh()
      await reload()
    }}
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
  .viewbar {
    display: flex;
    justify-content: flex-end;
    margin-bottom: 6px;
  }
  .viewsel {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 0.9rem;
    color: var(--muted);
  }
  .viewsel select {
    font: inherit;
    color: var(--text);
    padding: 6px 10px;
    border-radius: 999px;
    border: 1px solid var(--line);
    background: var(--surface);
  }
  .count {
    opacity: 0.7;
    font-size: 0.85em;
    margin-left: 2px;
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
    padding-right: 170px;
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
  .setins {
    max-width: 760px;
    margin: 0 0 0.75rem;
    padding: 0.75rem 1rem;
    border-radius: 1rem;
  }
  .setins .row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    justify-content: space-between;
  }
  .setins h3 {
    margin: 0;
    font-size: 1rem;
  }
  .setins .col {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .setins textarea {
    font: inherit;
    font-size: 1rem;
    padding: 0.4rem 0.6rem;
    border-radius: 0.6rem;
    border: 1px solid var(--line, rgba(0, 0, 0, 0.2));
    background: var(--surface, #fff);
    color: var(--text, inherit);
  }
  .itemnote {
    font-style: italic;
  }
  .scores :global(.side.notebtn) {
    right: 126px;
    bottom: auto;
    top: 50%;
    transform: translateY(-50%);
  }
  .welcome {
    margin: 0.75rem 0;
    padding: 1rem 1.25rem;
    border-radius: 1rem;
  }
  .welcome h2 {
    margin: 0 0 0.25rem;
    font-size: 1.25rem;
  }
  .welcome .how {
    margin: 0.5rem 0 0.75rem;
    padding-left: 1.25rem;
    line-height: 1.5;
  }
  .welcome .linklike {
    all: unset;
    cursor: pointer;
    text-decoration: underline;
    color: var(--accent);
  }
  .welcome .linklike:focus-visible,
  .trycard:focus-visible {
    outline: 0.125rem solid var(--accent);
    outline-offset: 0.125rem;
  }
  .trycard {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.25rem;
    width: 100%;
    min-height: 2.75rem;
    padding: 0.75rem 1rem;
    border-radius: 0.75rem;
    border: 0.0625rem solid var(--accent);
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
</style>
