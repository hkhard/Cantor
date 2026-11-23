<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { api, type ChannelInfo, type Opened, type ScoreEntry, type Strip, type ScoreSettings, type Setlist } from './lib/api'
  import { engrave, partNames, withoutParts, withNoteIds, noteAddress, type Engraved } from './lib/engrave'
  import Appearance from './lib/Appearance.svelte'
  import PdfView from './lib/PdfView.svelte'
  import Library from './lib/Library.svelte'
  import TrackPanel from './lib/TrackPanel.svelte'
  import VoicePanel from './lib/VoicePanel.svelte'
  import { scoreFileUrl, type PageMode } from './lib/pdf'
  import { t } from './lib/i18n.svelte'
  import { ask } from './lib/prompts.svelte'
  import Dialog from './lib/Dialog.svelte'
  import Icon from './lib/Icon.svelte'
  import { listen } from '@tauri-apps/api/event'
  import { openAnnotations, editAnnotations, STAMPS, type AnnDoc, type AnnCommand, type Tool, type Box } from './lib/annot'
  import AnnotLayer from './lib/AnnotLayer.svelte'

  let scores: ScoreEntry[] = $state([])
  let notice = $state('')
  let opened: Opened | null = $state(null)
  let engraved: Engraved | null = $state(null)
  let page = $state(1)
  let svg = $state('')
  let loading = $state('')
  let error = $state('')

  let playing = $state(false)
  let position = $state(0)
  let tempo = $state(100)
  let strips: Strip[] = $state([])
  let mine: number | null = $state(null)
  let follow = $state(true)
  // On narrow screens (iPad mini portrait) the mixer starts closed so the
  // music gets the full width.
  let mixerOpen = $state(window.innerWidth >= 1000)
  /** PLAY-09: the instrument / voice-set panel inside the mixer. */
  let voicesOpen = $state(false)
  let appearanceOpen = $state(false)
  /** 'pdf' = the original page, 'notation' = engraved from MusicXML. */
  let view: 'pdf' | 'notation' = $state('notation')
  let playingMeasure = $state(-1)
  // PLAY-07 loop: tap "Slinga", then the first and last bar.
  let loopPick: 'off' | 'a' | 'b' = $state('off')
  let loopA: number | null = $state(null)
  let loopB: number | null = $state(null)
  // PLAY-06
  let metronome = $state(false)
  let countIn = $state(false)
  let exportOpen = $state(false)
  let exporting = $state('')

  // ANN-01..03: annotation mode on the original page.
  let ann: AnnDoc | null = $state(null)
  let annotating = $state(false)
  let tool: Tool = $state('pen')
  let inkColor = $state('#1d4ed8')
  let annLayer = $state(0)
  let stamp = $state('Breath')
  let layersOpen = $state(false)
  const inkColors = ['#1d4ed8', '#b91c1c', '#15803d', '#1d1d1f']

  async function annEdit(c: AnnCommand) {
    try {
      ann = await editAnnotations('apply', c)
    } catch (e) {
      notice = String(e)
    }
  }
  async function annUndo(redo = false) {
    try {
      ann = await editAnnotations(redo ? 'redo' : 'undo')
    } catch {
      /* nothing to undo */
    }
  }
  async function addLayer() {
    const name = await ask(t('ann.layerName'), `${t('ann.layer')} ${(ann?.layers.length ?? 0) + 1}`)
    if (!name) return
    await annEdit({ op: 'AddLayer', name })
    if (ann) annLayer = ann.layers.length - 1
  }

  // Remembered per score (library store): mixer, my part, tempo, view,
  // choir, transposition (VIEW-05), hidden parts (VIEW-04), saved mixes (PLAY-04).
  let currentEntry: ScoreEntry | null = $state(null)
  let settings: ScoreSettings | null = $state(null)
  let parts: string[] = $state([])
  let moreOpen = $state(false)
  let pageMode: PageMode = $state((localStorage.getItem('cantor.pageMode') as PageMode) ?? 'single')
  /** Open from a setlist: position for previous/next piece (SET-01). */
  let setCtx: { list: Setlist; index: number } | null = $state(null)

  let persistTimer: number | undefined
  function persist() {
    if (!currentEntry || !settings) return
    settings.strips = strips.map((s) => ({ ...s, volume_db: s.volume_db === -Infinity ? -1000 : s.volume_db }))
    settings.my_part = mine
    settings.tempo_percent = tempo
    settings.view = view
    const id = currentEntry.file
    const snap = $state.snapshot(settings) as ScoreSettings
    clearTimeout(persistTimer)
    persistTimer = setTimeout(() => api.saveScoreSettings(id, snap).catch(() => {}), 300) as unknown as number
  }

  function notationXml(): string {
    return opened ? withoutParts(withNoteIds(opened.musicxml), settings?.hidden_parts ?? []) : ''
  }

  // LIB-06: correct OMR mistakes on the notation (user's own scores).
  let correcting = $state(false)
  let picked: { part: number; measure: number; note: number; id: string } | null = $state(null)
  function markPicked() {
    scoreEl?.querySelectorAll('.picked').forEach((el) => el.classList.remove('picked'))
    if (picked) scoreEl?.querySelector(`[data-id="${CSS.escape(picked.id)}"]`)?.classList.add('picked')
  }
  async function correct(edit: 'up' | 'down' | 'rest') {
    if (!picked || !currentEntry) return
    const keep = picked
    try {
      await api.correctNote(currentEntry.file, keep.part, keep.measure, keep.note, edit)
      const pageNow = page
      await open(currentEntry, opened?.choir === 'auto' ? undefined : opened?.choir, setCtx ?? undefined)
      view = 'notation'
      if (engraved && pageNow <= engraved.pages) {
        page = pageNow
        svg = engraved.render(page)
      }
      await tick()
      picked = edit === 'rest' ? null : keep
      markPicked()
    } catch (e) {
      exporting = String(e)
      setTimeout(() => (exporting = ''), 6000)
    }
  }

  async function setTranspose(n: number) {
    if (!settings || !currentEntry) return
    settings.transpose = Math.max(-12, Math.min(12, n))
    persist()
    const keepSet = setCtx
    await open(currentEntry, opened?.choir === 'auto' ? undefined : opened?.choir, keepSet)
  }

  async function toggleHidden(i: number) {
    if (!settings || !opened) return
    const h = new Set(settings.hidden_parts)
    if (h.has(i)) h.delete(i)
    else if (h.size < parts.length - 1) h.add(i)
    settings.hidden_parts = [...h].sort((a, b) => a - b)
    persist()
    lastSize = ''
    if (view === 'notation') await relayout()
    else if (scoreEl) await relayout()
    else {
      const w = Math.max(600, window.innerWidth - (mixerOpen ? 320 : 0))
      const h2 = Math.max(600, window.innerHeight - 140)
      engraved = await engrave(notationXml(), w, h2)
    }
  }

  function setPageMode(m: PageMode) {
    pageMode = m
    localStorage.setItem('cantor.pageMode', m)
  }

  async function savePreset() {
    if (!settings) return
    const name = await ask(t('mix.presetName'), `Mix ${settings.presets.length + 1}`)
    if (!name) return
    settings.presets.push({ name, strips: strips.map((s) => ({ ...s, volume_db: s.volume_db === -Infinity ? -1000 : s.volume_db })) })
    persist()
  }

  async function applyCustom(i: number) {
    const p = settings?.presets[i]
    if (!p) return
    for (let c = 0; c < strips.length && c < p.strips.length; c++) {
      const st = p.strips[c]
      strips[c] = { volume_db: st.volume_db <= -40 ? -Infinity : st.volume_db, mute: st.mute, solo: st.solo }
      await api.setChannel(c, { volumeDb: st.volume_db, mute: st.mute, solo: st.solo })
    }
    persist()
  }

  async function removePreset(i: number) {
    settings?.presets.splice(i, 1)
    persist()
  }

  async function stepSet(d: number) {
    if (!setCtx) return
    const items = setCtx.list.items
    for (let i = setCtx.index + d; i >= 0 && i < items.length; i += d) {
      const it = items[i]
      if (it.kind !== 'score') continue
      const s = scores.find((x) => x.file === it.score)
      if (s) {
        await open(s, undefined, { list: setCtx.list, index: i })
        return
      }
    }
  }

  let scoreEl: HTMLDivElement | undefined = $state()
  /** ANN-04: bars of the engraved page as boxes in fractions of the score
   *  pane (page = notation page), so notes drawn on the notation are anchored
   *  to bars and follow them through re-layout. */
  let notationBoxes: (Box | null)[] = $state([])
  let paneSize = $state({ w: 0, h: 0 })
  function measureNotation() {
    if (!scoreEl || !engraved) return
    const host = scoreEl.getBoundingClientRect()
    paneSize = { w: host.width, h: host.height }
    const out: (Box | null)[] = []
    for (const [i, loc] of engraved.measurePage) {
      out[i] = null
      if (loc.page !== page) continue
      const g = scoreEl.querySelector(`g.measure[data-id="${CSS.escape(loc.id)}"]`) as SVGGElement | null
      if (!g) continue
      // Staff lines span the whole bar; notes and lyrics can stick out.
      const staves = g.querySelectorAll('g.staff')
      let r = g.getBoundingClientRect()
      if (staves.length) {
        const rs = [...staves].map((st) => st.getBoundingClientRect())
        const top = Math.min(...rs.map((q) => q.top))
        const bottom = Math.max(...rs.map((q) => q.bottom))
        r = new DOMRect(r.left, top, r.width, bottom - top)
      }
      if (r.width < 1 || r.height < 1) continue
      out[i] = {
        page,
        x: (r.left - host.left) / host.width,
        y: (r.top - host.top) / host.height,
        w: r.width / host.width,
        h: r.height / host.height,
      }
    }
    notationBoxes = out
  }
  $effect(() => {
    // Re-measure whenever a page is rendered.
    void svg
    void page
    tick().then(() => requestAnimationFrame(measureNotation))
  })
  let currentMeasure = -1
  let poll: number | undefined

  onMount(async () => {
    try {
      scores = await api.listScores()
      // Files shared to Cantor from other apps (share sheet, AirDrop, Files).
      listen<string[]>('library-changed', async (ev) => {
        scores = await api.listScores()
        const added = ev.payload ?? []
        notice = added.length ? t('lib.imported', { n: added.length }) : ''
        // A single shared score opens straight away.
        if (added.length === 1) {
          const s = scores.find((x) => x.file === added[0])
          if (s) await open(s)
        }
      })
      // Demo/test hook: auto-open a score whose title contains this text,
      // set at build time (VITE_OPEN) or via ?open= in the URL.
      const want =
        new URLSearchParams(location.search).get('open') ?? import.meta.env.VITE_OPEN
      const hit = want && scores.find((s) => s.title.toLowerCase().includes(want.toLowerCase()))
      if (hit) await open(hit)
      if (hit && import.meta.env.VITE_AUTOPLAY) await toggle()
    } catch (e) {
      error = String(e)
    }
  })

  function fmt(s: number) {
    const m = Math.floor(s / 60)
    const r = Math.floor(s % 60)
    return `${m}:${r.toString().padStart(2, '0')}`
  }

  async function open(entry: ScoreEntry, choir?: string, set?: { list: Setlist; index: number }) {
    const tOpen = globalThis.performance.now()
    error = ''
    loading = t('read.opening', { title: entry.title })
    try {
      await stopPolling()
      setCtx = set ?? null
      if (currentEntry?.file !== entry.file || !settings) {
        settings = await api.scoreSettings(entry.file).catch(() => null)
      }
      currentEntry = entry
      if (!choir && settings && settings.choir !== 'auto') choir = settings.choir
      if (settings && choir) settings.choir = choir as ScoreSettings['choir']
      opened = await api.openScore(entry.file, choir, settings?.transpose ?? 0, settings?.concert_pitch ?? false)
      parts = opened.musicxml ? partNames(opened.musicxml) : []
      strips = opened.channels.map(() => ({ volume_db: 0, mute: false, solo: false }))
      mine = null
      view = opened.pdf_url ? 'pdf' : 'notation'
      if (settings) {
        if (settings.view === 'notation' || !opened.pdf_url) view = opened.musicxml ? (opened.pdf_url ? settings.view : 'notation') : 'pdf'
        if (settings.strips.length === opened.channels.length) {
          for (let c = 0; c < strips.length; c++) {
            const st = settings.strips[c]
            strips[c] = { volume_db: st.volume_db <= -40 ? -Infinity : st.volume_db, mute: st.mute, solo: st.solo }
            if (st.volume_db !== 0 || st.mute || st.solo)
              api.setChannel(c, { volumeDb: st.volume_db, mute: st.mute, solo: st.solo })
          }
        }
        if (settings.my_part !== null && settings.my_part < strips.length) mine = settings.my_part
      }
      annotating = false
      annLayer = 0
      ann = await openAnnotations(entry.file).catch(() => null)
      loopPick = 'off'
      loopA = null
      loopB = null
      if (metronome || countIn) api.setClick({ metronome, countIn })
      playingMeasure = -1
      playing = false
      position = 0
      tempo = settings?.tempo_percent ?? 100
      if (tempo !== 100 && opened.channels.length) api.setTempo(tempo / 100)
      if (!opened.musicxml) {
        // PDF only: show the page, no notation; a backing track still plays.
        engraved = null
        svg = ''
        if (opened.channels.length) startPolling()
        return
      }
      loading = t('read.engraving')
      await tick()
      // In PDF view the notation pane is not mounted: size from the window.
      const w = scoreEl?.clientWidth ?? Math.max(600, window.innerWidth - (mixerOpen ? 320 : 0))
      const h = scoreEl?.clientHeight ?? Math.max(600, window.innerHeight - 140)
      const tEngrave = globalThis.performance.now()
      engraved = await engrave(notationXml(), w, h)
      api.perfMark(`engrave ${entry.title} (${engraved.pages} pages, ${opened.measures.length} bars)`, globalThis.performance.now() - tEngrave).catch(() => {})
      lastSize = `${w}x${h}`
      page = 1
      currentMeasure = -1
      svg = engraved.render(1)
      await tick()
      api.perfMark(`first page ${entry.title}`, globalThis.performance.now() - tOpen).catch(() => {})
      if (opened.channels.length) startPolling()
    } catch (e) {
      error = String(e)
    } finally {
      loading = ''
    }
  }

  function close() {
    persist()
    setCtx = null
    stopPolling()
    api.pause().catch(() => {})
    opened = null
    engraved = null
    svg = ''
  }

  async function stopPolling() {
    if (poll) clearInterval(poll)
    poll = undefined
  }

  function startPolling() {
    stopPolling()
    poll = setInterval(async () => {
      try {
        const st = await api.status()
        playing = st.playing
        position = st.position
        // PERF-04: page turns from the backing track's markers.
        for (const p of st.turns ?? []) gotoPage(p)
        updateCursor()
      } catch {
        /* score closed */
      }
    }, 80) as unknown as number
  }

  /** Written measure sounding at `position` (repeats unfolded by the core). */
  function measureAt(t: number): number {
    if (!opened) return -1
    const ms = opened.measures
    let lo = 0
    let hi = ms.length - 1
    let found = -1
    while (lo <= hi) {
      const mid = (lo + hi) >> 1
      if (ms[mid][0] <= t + 1e-6) {
        found = mid
        lo = mid + 1
      } else hi = mid - 1
    }
    return found < 0 ? -1 : ms[found][1]
  }

  async function updateCursor() {
    playingMeasure = measureAt(position)
    if (!engraved || view !== 'notation') return
    const m = measureAt(position)
    if (m === currentMeasure) return
    currentMeasure = m
    const loc = engraved.measurePage.get(m)
    if (!loc) return
    if (follow && loc.page !== page) {
      page = loc.page
      svg = engraved.render(page)
      await tick()
    }
    highlight(loc.id)
  }

  /** Tint the playing measure and draw a band behind it (PLAY-08 cursor). */
  function highlight(id: string) {
    scoreEl?.querySelectorAll('.playing').forEach((el) => el.classList.remove('playing'))
    scoreEl?.querySelectorAll('rect.cursor').forEach((el) => el.remove())
    const el = scoreEl?.querySelector(`g.measure[data-id="${CSS.escape(id)}"]`) as SVGGElement | null
    if (!el) return
    el.classList.add('playing')
    try {
      const b = el.getBBox()
      const r = document.createElementNS('http://www.w3.org/2000/svg', 'rect')
      r.setAttribute('class', 'cursor')
      r.setAttribute('x', String(b.x - 40))
      r.setAttribute('y', String(b.y - 40))
      r.setAttribute('width', String(b.width + 80))
      r.setAttribute('height', String(b.height + 80))
      r.setAttribute('rx', '60')
      el.parentNode?.insertBefore(r, el)
    } catch {
      /* not rendered yet */
    }
  }

  async function toggle() {
    if (playing) await api.pause()
    else await api.play()
    playing = !playing
  }

  async function toStart() {
    await api.seek(0)
    position = 0
    currentMeasure = -1
    if (engraved && page !== 1) {
      page = 1
      svg = engraved.render(1)
    }
    updateCursor()
  }

  async function scrub(e: Event) {
    const v = Number((e.target as HTMLInputElement).value)
    await api.seek(v)
    position = v
    updateCursor()
  }

  /** Tap a measure in the score to play from there. */
  async function tapScore(e: MouseEvent) {
    if (!opened || !engraved) return
    if (correcting) {
      // Nearest note head to the tap (glyphs are thin and the tap target is
      // often the staff or the page, not the note itself).
      let el = (e.target as Element).closest('[data-id^="cn-"]')
      if (!el && scoreEl) {
        let best = 28 * 28 // within 28 px
        for (const n of scoreEl.querySelectorAll('g.note[data-id^="cn-"], g.rest[data-id^="cn-"], g.mRest[data-id^="cn-"]')) {
          const r = n.getBoundingClientRect()
          const dx = Math.max(r.left - e.clientX, 0, e.clientX - r.right)
          const dy = Math.max(r.top - e.clientY, 0, e.clientY - r.bottom)
          const d = dx * dx + dy * dy
          if (d < best) {
            best = d
            el = n
          }
        }
      }
      const id = el?.getAttribute('data-id') ?? null
      const a = noteAddress(id)
      picked = a && id ? { ...a, id } : null
      markPicked()
      return
    }
    const g = (e.target as Element).closest('g.measure')
    if (!g) return
    const id = g.getAttribute('data-id') ?? g.id
    let written = -1
    for (const [i, loc] of engraved.measurePage) if (loc.id === id) written = i
    if (written >= 0) await seekMeasure(written)
  }

  async function setTempo(e: Event) {
    tempo = Number((e.target as HTMLInputElement).value)
    await api.setTempo(tempo / 100)
    persist()
  }

  function turn(d: number) {
    if (!engraved) return
    const p = Math.min(engraved.pages, Math.max(1, page + d))
    if (p !== page) {
      page = p
      svg = engraved.render(p)
      tick().then(() => {
        const loc = engraved?.measurePage.get(currentMeasure)
        if (loc) highlight(loc.id)
      })
    }
  }

  async function setMute(i: number) {
    strips[i].mute = !strips[i].mute
    await api.setChannel(i, { mute: strips[i].mute })
    persist()
  }

  async function setSolo(i: number) {
    strips[i].solo = !strips[i].solo
    await api.setChannel(i, { solo: strips[i].solo })
    persist()
  }

  async function setVolume(i: number, e: Event) {
    const v = Number((e.target as HTMLInputElement).value)
    strips[i].volume_db = v <= -40 ? -Infinity : v
    await api.setChannel(i, { volumeDb: v <= -40 ? -1000 : v })
    persist()
  }

  async function preset(p: 'my_part' | 'without_me' | 'balanced') {
    if (p !== 'balanced' && mine === null) return
    strips = await api.applyPreset(p, mine ?? undefined)
    persist()
  }

  async function changeChoir(e: Event) {
    if (!opened) return
    const v = (e.target as HTMLSelectElement).value
    if (settings) settings.choir = v as ScoreSettings['choir']
    persist()
    if (currentEntry) await open(currentEntry, v === 'auto' ? undefined : v, setCtx ?? undefined)
  }

  function roleClass(c: ChannelInfo) {
    return c.role.toLowerCase()
  }

  /** Play from the first time a written measure sounds (repeats unfolded). */
  async function seekMeasure(written: number) {
    if (loopPick === 'a') {
      loopA = written
      loopB = null
      loopPick = 'b'
      markLoop()
      return
    }
    if (loopPick === 'b' && loopA !== null) {
      loopB = written
      loopPick = 'off'
      const [a, b] = [Math.min(loopA, written), Math.max(loopA, written)]
      loopA = a
      loopB = b
      await api.setLoop(a, b)
      markLoop()
      return
    }
    const first = opened?.measures.find((m) => m[1] === written)
    if (!first) return
    await api.seek(first[0])
    position = first[0]
    currentMeasure = -1
    updateCursor()
  }

  async function toggleLoop() {
    if (loopPick !== 'off' || loopB !== null) {
      loopPick = 'off'
      loopA = null
      loopB = null
      await api.setLoop()
      markLoop()
    } else {
      loopPick = 'a'
    }
  }

  /** Outline the loop's bars in the notation view. */
  function markLoop() {
    scoreEl?.querySelectorAll('g.measure.inloop').forEach((el) => el.classList.remove('inloop'))
    if (!engraved || loopA === null) return
    const b = loopB ?? loopA
    for (let m = loopA; m <= b; m++) {
      const loc = engraved.measurePage.get(m)
      if (loc) scoreEl?.querySelector(`g.measure[data-id="${CSS.escape(loc.id)}"]`)?.classList.add('inloop')
    }
  }

  let audioFormat: 'm4a' | 'wav' = $state((localStorage.getItem('cantor.audioFormat') as 'm4a' | 'wav') ?? 'm4a')
  async function exportAudio(what: 'mix' | 'parts') {
    exportOpen = false
    exporting = what === 'mix' ? t('tr.exportingMix') : t('tr.exportingParts')
    try {
      const names = await api.exportAudio(what, audioFormat)
      exporting = t('tr.exported', { n: names.length })
    } catch (e) {
      exporting = t('tr.exportFailed', { e: String(e) })
    }
    setTimeout(() => (exporting = ''), 6000)
  }

  async function shareItem(kind: 'pdf' | '4sc') {
    exportOpen = false
    if (!currentEntry) return
    try {
      exporting = t('share.done', { name: await api.exportItem(kind, currentEntry.file) })
    } catch (e) {
      exporting = t('share.failed', { e: String(e) })
    }
    setTimeout(() => (exporting = ''), 6000)
  }

  async function toggleMetronome() {
    metronome = !metronome
    await api.setClick({ metronome })
  }

  async function toggleCountIn() {
    countIn = !countIn
    await api.setClick({ countIn })
  }

  /** Re-engrave for the current pane size (rotation, mixer shown/hidden),
   *  keeping the playing measure in view. */
  let relayoutTimer: number | undefined
  let lastSize = ''
  function scheduleRelayout() {
    clearTimeout(relayoutTimer)
    relayoutTimer = setTimeout(relayout, 150) as unknown as number
  }
  async function relayout() {
    if (!opened || !scoreEl || view !== 'notation') return
    const w = scoreEl.clientWidth
    const h = scoreEl.clientHeight
    const key = `${w}x${h}`
    if (key === lastSize || w < 200) return
    lastSize = key
    engraved = await engrave(notationXml(), w, h)
    const loc = engraved.measurePage.get(currentMeasure >= 0 ? currentMeasure : 0)
    page = loc?.page ?? 1
    svg = engraved.render(page)
    await tick()
    if (loc && currentMeasure >= 0) highlight(loc.id)
  }

  $effect(() => {
    if (!scoreEl) return
    const ro = new ResizeObserver(scheduleRelayout)
    ro.observe(scoreEl)
    return () => ro.disconnect()
  })

  async function switchView(v: 'pdf' | 'notation') {
    view = v
    persist()
    currentMeasure = -1
    if (v === 'notation' && engraved) {
      await tick()
      svg = engraved.render(page)
      await tick()
      updateCursor()
    }
  }

  /** PERF-02: Bluetooth page turners (AirTurn, PageFlip, ...) are
   *  keyboards; each key maps to an action the user can change. */
  type PedalAction = 'next' | 'prev' | 'play' | 'none'
  const pedalKeys = ['ArrowRight', 'ArrowLeft', 'ArrowDown', 'ArrowUp', 'PageDown', 'PageUp', 'Space', 'Enter']
  const defaultPedals: Record<string, PedalAction> = {
    ArrowRight: 'next',
    ArrowDown: 'next',
    PageDown: 'next',
    ArrowLeft: 'prev',
    ArrowUp: 'prev',
    PageUp: 'prev',
    Space: 'play',
    Enter: 'none',
  }
  let pedals: Record<string, PedalAction> = $state(
    (() => {
      try {
        return { ...defaultPedals, ...JSON.parse(localStorage.getItem('cantor.pedals') ?? '{}') }
      } catch {
        return { ...defaultPedals }
      }
    })(),
  )
  let pedalsOpen = $state(false)
  function setPedal(key: string, a: PedalAction) {
    pedals[key] = a
    localStorage.setItem('cantor.pedals', JSON.stringify(pedals))
  }

  // PERF-01: performance mode hides everything but the music and makes the
  // left/right halves of the screen page-turn zones; editing is locked.
  let performance = $state(false)

  let pdfView: { turn: (d: number) => void; goto: (p: number) => void; pageInfo: () => { page: number; pages: number } } | undefined = $state()
  function gotoPage(p: number) {
    if (view === 'pdf' && pdfView) pdfView.goto(p)
    else if (engraved && p >= 1 && p <= engraved.pages && p !== page) {
      page = p
      svg = engraved.render(p)
    }
  }
  function currentPage(): number {
    return view === 'pdf' && pdfView ? pdfView.pageInfo().page : page
  }
  function turnAny(d: number) {
    if (view === 'pdf' && pdfView) pdfView.turn(d)
    else turn(d)
  }

  function onKey(e: KeyboardEvent) {
    if (!opened) return
    const target = e.target as HTMLElement | null
    if (target && ['INPUT', 'SELECT', 'TEXTAREA'].includes(target.tagName)) return
    const action = pedals[e.code]
    if (!action || action === 'none') {
      if (e.code === 'Escape' && performance) performance = false
      return
    }
    e.preventDefault()
    if (action === 'next') turnAny(1)
    else if (action === 'prev') turnAny(-1)
    else if (action === 'play' && opened.channels.length) toggle()
  }
</script>

<svelte:window onkeydown={onKey} />

{#if !opened}
  <Library
    {scores}
    bind:notice
    {error}
    {loading}
    onopen={(s, set) => open(s, undefined, set)}
    onrefresh={async () => {
      scores = await api.listScores()
    }}
  />
{:else}
  <main class="reader" class:performance>
    {#if performance}
      <!-- PERF-01: big turn zones, no editing; a small exit button. -->
      <button class="turnzone left" onclick={() => turnAny(-1)} aria-label={t('read.prev')}></button>
      <button class="turnzone right" onclick={() => turnAny(1)} aria-label={t('read.next')}></button>
      <button class="perfexit glass" onclick={() => (performance = false)}>{t('read.performExit')}</button>
    {/if}
    <nav class="top glass">
      <button class="icon" onclick={close} aria-label={t('read.library')}><Icon name="back" size={20} /> <span class="wide">{t('read.library')}</span></button>
      {#if setCtx}
        <button class="icon" onclick={() => stepSet(-1)} disabled={setCtx.index === 0} aria-label="◀">◀</button>
      {/if}
      <h2>{opened.title}{#if setCtx}<span class="setpos"> · {setCtx.list.name} {setCtx.index + 1}/{setCtx.list.items.length}</span>{/if}</h2>
      {#if setCtx}
        <button class="icon" onclick={() => stepSet(1)} disabled={setCtx.index >= setCtx.list.items.length - 1} aria-label="▶">▶</button>
      {/if}
      {#if opened.pdf_url && opened.musicxml}
        <div class="seg" role="group" aria-label={t('read.pageMode')}>
          <button class:on={view === 'pdf'} aria-pressed={view === 'pdf'} onclick={() => switchView('pdf')}>{t('read.original')}</button>
          <button class:on={view === 'notation'} aria-pressed={view === 'notation'} onclick={() => switchView('notation')}>{t('read.notation')}</button>
        </div>
      {/if}
      {#if view === 'notation' && engraved && currentEntry?.user && !performance}
        <button class="icon" class:active={correcting} aria-pressed={correcting} onclick={() => { correcting = !correcting; picked = null; annotating = false; markPicked() }}
          >{correcting ? t('common.done') : t('read.correct')}</button
        >
      {/if}
      {#if ann && (view === 'pdf' ? !!opened.pdf_url : !!engraved)}
        <button class="icon annotate" class:active={annotating} aria-pressed={annotating} onclick={() => (annotating = !annotating)}
          >{annotating ? t('common.done') : t('read.annotate')}</button
        >
      {/if}
      <button class="icon" class:active={moreOpen} onclick={() => { moreOpen = !moreOpen; pedalsOpen = false; appearanceOpen = false }} aria-haspopup="dialog" aria-expanded={moreOpen} aria-label={t('read.more')}>•••</button>
      {#if opened.channels.length}
        <button class="icon" class:active={mixerOpen} onclick={() => (mixerOpen = !mixerOpen)} aria-label={mixerOpen ? t('read.hideMixer') : t('read.mixer')} aria-pressed={mixerOpen}
          ><Icon name="sliders" /> <span class="wide">{mixerOpen ? t('read.hideMixer') : t('read.mixer')}</span></button
        >
      {/if}
      {#if moreOpen}
        <div class="popover glass more">
          <button class="primary" onclick={() => { performance = true; mixerOpen = false; annotating = false; moreOpen = false }}>{t('read.perform')}</button>
          {#if opened.channels.length}
            <label class="row"
              >{t('read.choir')}
              <select value={opened.choir} onchange={changeChoir}>
                <option value="auto">{t('read.choirAuto')}</option>
                <option value="mixed">{t('read.choirMixed')}</option>
                <option value="male">{t('read.choirMale')}</option>
                <option value="female">{t('read.choirFemale')}</option>
              </select>
            </label>
          {/if}
          {#if opened.musicxml}
            <div class="row">
              <span>{t('read.transpose')}</span>
              <span class="stepper">
                <button onclick={() => setTranspose((settings?.transpose ?? 0) - 1)} aria-label="−1">−</button>
                <span class="val">{(settings?.transpose ?? 0) > 0 ? '+' : ''}{settings?.transpose ?? 0}</span>
                <button onclick={() => setTranspose((settings?.transpose ?? 0) + 1)} aria-label="+1">+</button>
                {#if settings?.transpose}<button onclick={() => setTranspose(0)}>0</button>{/if}
              </span>
            </div>
            {#if opened.has_transposing}
              <label class="check"
                ><input
                  type="checkbox"
                  checked={settings?.concert_pitch ?? false}
                  onchange={async () => {
                    if (!settings || !currentEntry) return
                    settings.concert_pitch = !settings.concert_pitch
                    persist()
                    await open(currentEntry, opened?.choir === 'auto' ? undefined : opened?.choir, setCtx ?? undefined)
                  }}
                /> {t('read.concertPitch')}</label
              >
            {/if}
            {#if parts.length > 1}
              <div class="col">
                <span>{t('read.parts')}</span>
                {#each parts as name, i}
                  <label class="check"
                    ><input type="checkbox" checked={!settings?.hidden_parts.includes(i)} onchange={() => toggleHidden(i)} /> {name}</label
                  >
                {/each}
              </div>
            {/if}
          {/if}
          {#if opened.pdf_url}
            <label class="row"
              >{t('read.pageMode')}
              <select value={pageMode} onchange={(e) => setPageMode((e.target as HTMLSelectElement).value as PageMode)}>
                <option value="single">{t('read.pageSingle')}</option>
                <option value="two">{t('read.pageTwo')}</option>
                <option value="half">{t('read.pageHalf')}</option>
                <option value="scroll">{t('read.pageScroll')}</option>
              </select>
            </label>
          {/if}
          {#if currentEntry}
            <TrackPanel
              file={currentEntry.file}
              track={opened.track}
              page={currentPage()}
              {position}
              onchanged={async () => {
                if (currentEntry) await open(currentEntry, opened?.choir === 'auto' ? undefined : opened?.choir, setCtx ?? undefined)
              }}
            />
          {/if}
          <div class="row">
            <button onclick={() => (pedalsOpen = !pedalsOpen)} class:on={pedalsOpen}>{t('read.pedal')}</button>
            <button onclick={() => (appearanceOpen = !appearanceOpen)} class:on={appearanceOpen}><Icon name="appearance" /> {t('read.appearance')}</button>
          </div>
          {#if appearanceOpen}<Appearance />{/if}
          {#if pedalsOpen}
            <div class="col pedals">
              <p class="hint">{t('read.pedalHint')}</p>
              {#each pedalKeys as k}
                <label class="row"
                  >{k}
                  <select value={pedals[k] ?? 'none'} onchange={(e) => setPedal(k, (e.target as HTMLSelectElement).value as PedalAction)}>
                    <option value="next">{t('read.next')}</option>
                    <option value="prev">{t('read.prev')}</option>
                    <option value="play">{t('read.playPause')}</option>
                    <option value="none">{t('read.nothing')}</option>
                  </select>
                </label>
              {/each}
            </div>
          {/if}
        </div>
      {/if}
    </nav>

    {#if correcting && view === 'notation'}
      <div class="annbar glass" role="toolbar" aria-label={t('read.correct')}>
        <span class="hint">{picked ? t('fix.picked', { bar: picked.measure + 1 }) : t('fix.hint')}</span>
        <button class="icon" disabled={!picked} onclick={() => correct('up')} aria-label={t('fix.up')}>↑ {t('fix.up')}</button>
        <button class="icon" disabled={!picked} onclick={() => correct('down')} aria-label={t('fix.down')}>↓ {t('fix.down')}</button>
        <button class="icon" disabled={!picked} onclick={() => correct('rest')}>{t('fix.rest')}</button>
      </div>
    {/if}

    {#if annotating && ann}
      <div class="annbar glass" role="toolbar">
        <div class="seg" role="group">
          <button class:on={tool === 'pen'} aria-pressed={tool === 'pen'} onclick={() => (tool = 'pen')}>{t('ann.pen')}</button>
          <button class:on={tool === 'highlighter'} aria-pressed={tool === 'highlighter'} onclick={() => (tool = 'highlighter')}>{t('ann.highlighter')}</button>
          <button class:on={tool === 'eraser'} aria-pressed={tool === 'eraser'} onclick={() => (tool = 'eraser')}>{t('ann.eraser')}</button>
          <button class:on={tool === 'text'} aria-pressed={tool === 'text'} onclick={() => (tool = 'text')}>{t('ann.text')}</button>
          <button class:on={tool === 'stamp'} aria-pressed={tool === 'stamp'} onclick={() => (tool = 'stamp')}>{t('ann.stamp')}</button>
        </div>
        {#if tool === 'pen' || tool === 'text'}
          <span class="colors" role="group" aria-label={t('ann.color')}>
            {#each inkColors as c}
              <button class="swatch" class:on={inkColor === c} aria-pressed={inkColor === c} style="background:{c}" onclick={() => (inkColor = c)} aria-label="{t('ann.color')} {c}"></button>
            {/each}
          </span>
        {/if}
        {#if tool === 'stamp'}
          <span class="stamps" role="group" aria-label={t('ann.stamp')}>
            {#each STAMPS as st}
              <button class:on={stamp === st.id} aria-pressed={stamp === st.id} aria-label={st.name} onclick={() => (stamp = st.id)}>{st.label}</button>
            {/each}
          </span>
        {/if}
        <button class="icon" onclick={() => annUndo()} aria-label={t('ann.undo')}>↶</button>
        <button class="icon" onclick={() => annUndo(true)} aria-label={t('ann.redo')}>↷</button>
        <span class="exportwrap">
          <button class="icon" class:active={layersOpen} onclick={() => (layersOpen = !layersOpen)}
            >{t('ann.layer')}: {ann.layers[annLayer]?.name ?? ''}</button
          >
          {#if layersOpen}
            <span class="menu glass layers">
              {#each ann.layers as l, i}
                <span class="layerrow">
                  <input
                    type="checkbox"
                    checked={l.visible}
                    onchange={() => annEdit({ op: 'SetLayerVisible', layer: i, visible: !l.visible })}
                    aria-label={l.name}
                  />
                  <button class:on={annLayer === i} onclick={() => { annLayer = i; layersOpen = false }}>{l.name}</button>
                </span>
              {/each}
              <button onclick={() => { layersOpen = false; addLayer() }}>{t('ann.newLayer')}</button>
            </span>
          {/if}
        </span>
      </div>
    {/if}

    <div class="body">
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      {#if view === 'pdf' && opened.pdf_url}
        <div class="score pdf">
          <PdfView
            bind:this={pdfView}
            url={scoreFileUrl(opened.pdf_url)}
            boxes={opened.measure_boxes}
            measure={playingMeasure}
            {follow}
            onseek={seekMeasure}
            loop={loopA !== null ? [loopA, loopB ?? loopA] : null}
            mode={performance && pageMode === 'scroll' ? 'single' : pageMode}
            {ann}
            tool={annotating && !performance ? tool : null}
            {inkColor}
            layer={annLayer}
            {stamp}
            onedit={annEdit}
          />
        </div>
      {:else}
      <div class="score" bind:this={scoreEl} onclick={tapScore} role="region" aria-label={t('a11y.notation', { n: page, of: engraved?.pages ?? 1 })}>
        {#if loading}<p class="loading">{loading}</p>{/if}
        {#if error}<p class="error">{error}</p>{/if}
        {@html svg}
        {#if engraved && paneSize.w > 0}
          <AnnotLayer
            doc={ann}
            {page}
            w={paneSize.w}
            h={paneSize.h}
            tool={annotating && !performance ? tool : null}
            color={inkColor}
            layer={annLayer}
            {stamp}
            onedit={annEdit}
            boxes={notationBoxes}
            measureOnly
          />
        {/if}
        {#if engraved && engraved.pages > 1 && !(annotating && !performance)}
          <button class="pageturn left" onclick={(e) => { e.stopPropagation(); turn(-1) }} aria-label={t('read.prev')}></button>
          <button class="pageturn right" onclick={(e) => { e.stopPropagation(); turn(1) }} aria-label={t('read.next')}></button>
        {/if}
      </div>
      {/if}

      {#if mixerOpen && opened.channels.length}
        <aside class="mixer glass">
          <div class="presets">
            <button onclick={() => preset('my_part')} disabled={mine === null}>{t('mix.myPart')}</button>
            <button onclick={() => preset('without_me')} disabled={mine === null}>{t('mix.withoutMe')}</button>
            <button onclick={() => preset('balanced')}>{t('mix.balanced')}</button>
          </div>
          <div class="presets custom">
            {#each settings?.presets ?? [] as p, i}
              <span class="cp"
                ><button onclick={() => applyCustom(i)}>{p.name}</button><button class="x" onclick={() => removePreset(i)} aria-label="{t('common.delete')} {p.name}">×</button></span
              >
            {/each}
            <button onclick={savePreset}>{t('mix.savePreset')}</button>
          </div>
          {#each opened.channels as c, i (i)}
            <div class="strip {roleClass(c)}" class:mine={mine === i}>
              <button class="name" aria-pressed={mine === i} onclick={() => { mine = mine === i ? null : i; persist() }} title={t('mix.hint')}>
                <span class="label">{c.label}</span>
                <span class="inst">{c.instrument}</span>
              </button>
              <input
                type="range"
                min="-40"
                max="6"
                step="1"
                value={strips[i].volume_db === -Infinity ? -40 : strips[i].volume_db}
                oninput={(e) => setVolume(i, e)}
                aria-label={t('mix.volume', { label: c.label })}
              />
              <button class="m" class:on={strips[i].mute} aria-pressed={strips[i].mute} aria-label="{t('mix.mute')} {c.label}" onclick={() => setMute(i)}>M</button>
              <button class="s" class:on={strips[i].solo} aria-pressed={strips[i].solo} aria-label="{t('mix.solo')} {c.label}" onclick={() => setSolo(i)}>S</button>
              <button class="pipe" onclick={() => api.pitchPipe(i)} aria-label="{t('mix.pipe')} {c.label}">♪</button>
            </div>
          {/each}
          <button class="chip vstoggle" aria-expanded={voicesOpen} onclick={() => (voicesOpen = !voicesOpen)}>{t('vs.edit')}</button>
          {#if voicesOpen && currentEntry}
            {#key currentEntry.file}
              <VoicePanel
                file={currentEntry.file}
                bind:channels={opened.channels}
                choir={opened.voice_choir ?? opened.choir}
                onreload={() => currentEntry && open(currentEntry, undefined, setCtx ?? undefined)}
              />
            {/key}
          {/if}
          <p class="hint">{t('mix.hint')}</p>
        </aside>
      {/if}
    </div>

    {#if !opened.channels.length}
      <footer class="transport glass silent">
        <span>{t('read.pdfOnly')}</span>
        {#if opened.pdf_url && currentEntry}
          <button class="chip" onclick={() => shareItem('pdf')}>{t('share.pdf')}</button>
          <button class="chip" onclick={() => shareItem('4sc')}>{t('share.4sc')}</button>
        {/if}
        {#if exporting}<span class="exporting">{exporting}</span>{/if}
      </footer>
    {:else}
    <footer class="transport glass">
      <button class="icon" onclick={toStart} aria-label={t('tr.start')}><Icon name="start" /></button>
      <button class="play" onclick={toggle} aria-label={playing ? t('tr.pause') : t('tr.play')}><Icon name={playing ? 'pause' : 'play'} size={24} /></button>
      <span class="time">{fmt(position)}</span>
      <input class="scrub" type="range" min="0" max={opened.duration} step="0.1" value={position} onchange={scrub} aria-label={t('tr.position')} />
      <span class="time">{fmt(opened.duration)}</span>
      <label class="tempo"
        >{t('tr.tempo')} {tempo}%
        <input type="range" min="25" max="200" step="5" value={tempo} oninput={setTempo} />
      </label>
      <button class="chip" onclick={() => api.pitchPipe()} title={t('tr.pipeHint')}>♪ {t('tr.pipe')}</button>
      <button
        class="chip"
        class:on={loopPick !== 'off' || loopB !== null}
        aria-pressed={loopPick !== 'off' || loopB !== null}
        onclick={toggleLoop}
        aria-label={t('tr.loop')}
        ><Icon name="loop" size={18} />
        {#if loopPick === 'a'}{t('tr.loopA')}{:else if loopPick === 'b'}{t('tr.loopB')}{:else if loopA !== null && loopB !== null}{t('tr.bars', { a: loopA + 1, b: loopB + 1 })}{:else}{t('tr.loop')}{/if}</button
      >
      <button class="chip" class:on={metronome} aria-pressed={metronome} onclick={toggleMetronome} aria-label={t('tr.click')}
        ><Icon name="metronome" size={18} /> {t('tr.click')}</button
      >
      <button class="chip" class:on={countIn} aria-pressed={countIn} onclick={toggleCountIn}>{t('tr.countIn')}</button>
      <span class="exportwrap">
        <button class="chip" onclick={() => (exportOpen = !exportOpen)} aria-haspopup="menu" aria-expanded={exportOpen}>{t('tr.export')}</button>
        {#if exportOpen}
          <span class="menu glass" role="menu">
            <button role="menuitem" onclick={() => exportAudio('mix')}>{t('tr.exportMix')}</button>
            <button role="menuitem" onclick={() => exportAudio('parts')}>{t('tr.exportParts')}</button>
            <span class="fmt" role="group" aria-label={t('tr.format')}>
              {#each ['m4a', 'wav'] as const as f}
                <button
                  role="menuitemradio"
                  class:on={audioFormat === f}
                  aria-checked={audioFormat === f}
                  onclick={() => {
                    audioFormat = f
                    localStorage.setItem('cantor.audioFormat', f)
                  }}>{f === 'm4a' ? t('tr.formatM4a') : t('tr.formatWav')}</button
                >
              {/each}
            </span>
            {#if opened.pdf_url && currentEntry}
              <button role="menuitem" onclick={() => shareItem('pdf')}>{t('share.pdf')}</button>
              <button role="menuitem" onclick={() => shareItem('4sc')}>{t('share.4sc')}</button>
            {/if}
          </span>
        {/if}
      </span>
      {#if exporting}<span class="exporting">{exporting}</span>{/if}
      <label class="follow"><input type="checkbox" bind:checked={follow} /> {t('tr.follow')}</label>
      {#if engraved && view === 'notation'}<span class="page">{page}/{engraved.pages}</span>{/if}
    </footer>
    {/if}
  </main>
{/if}

<Dialog />
