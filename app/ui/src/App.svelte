<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { api, type ChannelInfo, type Opened, type ScoreEntry, type Strip } from './lib/api'
  import { engrave, type Engraved } from './lib/engrave'
  import Appearance from './lib/Appearance.svelte'
  import PdfView from './lib/PdfView.svelte'
  import { scoreFileUrl } from './lib/pdf'
  import Icon from './lib/Icon.svelte'
  import { listen } from '@tauri-apps/api/event'

  let scores: ScoreEntry[] = $state([])
  let notice = $state('')
  let importing = $state(false)
  let fileInput: HTMLInputElement | undefined = $state()
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

  let scoreEl: HTMLDivElement | undefined = $state()
  let currentMeasure = -1
  let poll: number | undefined

  onMount(async () => {
    try {
      scores = await api.listScores()
      // Files shared to Cantor from other apps (share sheet, AirDrop, Files).
      listen<string[]>('library-changed', async (ev) => {
        scores = await api.listScores()
        const added = ev.payload ?? []
        notice = added.length
          ? `Importerade ${added.length} ${added.length === 1 ? 'fil' : 'filer'}`
          : ''
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

  async function open(entry: ScoreEntry, choir?: string) {
    error = ''
    loading = `Öppnar ${entry.title} …`
    try {
      await stopPolling()
      opened = await api.openScore(entry.file, choir)
      strips = opened.channels.map(() => ({ volume_db: 0, mute: false, solo: false }))
      mine = null
      view = opened.pdf_url ? 'pdf' : 'notation'
      loopPick = 'off'
      loopA = null
      loopB = null
      if (metronome || countIn) api.setClick({ metronome, countIn })
      playingMeasure = -1
      playing = false
      position = 0
      tempo = 100
      if (!opened.musicxml) {
        // PDF only: show the page, no sound or notation.
        engraved = null
        svg = ''
        return
      }
      loading = 'Sätter noterna …'
      await tick()
      // In PDF view the notation pane is not mounted: size from the window.
      const w = scoreEl?.clientWidth ?? Math.max(600, window.innerWidth - (mixerOpen ? 320 : 0))
      const h = scoreEl?.clientHeight ?? Math.max(600, window.innerHeight - 140)
      engraved = await engrave(opened.musicxml, w, h)
      lastSize = `${w}x${h}`
      page = 1
      currentMeasure = -1
      svg = engraved.render(1)
      if (opened.channels.length) startPolling()
    } catch (e) {
      error = String(e)
    } finally {
      loading = ''
    }
  }

  function close() {
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
  }

  async function setSolo(i: number) {
    strips[i].solo = !strips[i].solo
    await api.setChannel(i, { solo: strips[i].solo })
  }

  async function setVolume(i: number, e: Event) {
    const v = Number((e.target as HTMLInputElement).value)
    strips[i].volume_db = v <= -40 ? -Infinity : v
    await api.setChannel(i, { volumeDb: v <= -40 ? -1000 : v })
  }

  async function preset(p: 'my_part' | 'without_me' | 'balanced') {
    if (p !== 'balanced' && mine === null) return
    strips = await api.applyPreset(p, mine ?? undefined)
  }

  async function changeChoir(e: Event) {
    if (!opened) return
    const v = (e.target as HTMLSelectElement).value
    const entry = scores.find((s) => s.title === opened!.title)
    if (entry) await open(entry, v === 'auto' ? undefined : v)
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
    engraved = await engrave(opened.musicxml, w, h)
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
    currentMeasure = -1
    if (v === 'notation' && engraved) {
      await tick()
      svg = engraved.render(page)
      await tick()
      updateCursor()
    }
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
    scores = await api.listScores()
    notice = [
      added.length ? `Importerade ${added.length} ${added.length === 1 ? 'fil' : 'filer'}` : '',
      ...failed,
    ]
      .filter(Boolean)
      .join(' · ')
  }

  async function removeScore(s: ScoreEntry) {
    if (!confirm(`Ta bort "${s.title}" från ditt bibliotek?`)) return
    await api.deleteScore(s.file)
    scores = await api.listScores()
  }

  function onKey(e: KeyboardEvent) {
    if (!opened) return
    if (e.code === 'Space') {
      e.preventDefault()
      toggle()
    } else if (e.code === 'ArrowRight' || e.code === 'PageDown') turn(1)
    else if (e.code === 'ArrowLeft' || e.code === 'PageUp') turn(-1)
  }
</script>

<svelte:window onkeydown={onKey} />

{#if !opened}
  <main class="library">
    <header>
      <img class="mark" src="/cantor-mark.svg" alt="" width="64" height="64" />
      <div class="titles">
        <h1>Cantor</h1>
        <p class="sub">Noter som spelar alla stämmor</p>
      </div>
      <Appearance />
    </header>
    {#if error}<p class="error">{error}</p>{/if}
    {#if loading}<p class="loading">{loading}</p>{/if}
    <div class="libbar">
      <button class="chip on" onclick={() => fileInput?.click()} disabled={importing}
        >{importing ? 'Importerar …' : '＋ Importera'}</button
      >
      <input
        bind:this={fileInput}
        type="file"
        multiple
        accept=".mxl,.musicxml,.xml,.pdf,.4sc,.4ss,application/pdf,application/vnd.recordare.musicxml,application/vnd.recordare.musicxml+xml"
        style="display:none"
        onchange={importPicked}
      />
      {#if notice}<span class="notice">{notice}</span>{/if}
    </div>
    <ul class="scores">
      {#each scores as s (s.file)}
        <li>
          <button onclick={() => open(s)}>
            <span>{s.title}</span>
            <span class="badges">
              {#if s.playable}<span class="badge">♪ spelar</span>{/if}
              {#if s.pdf}<span class="badge">PDF</span>{/if}
              {#if s.user}<span class="badge mine">Mina</span>{/if}
            </span>
          </button>
          {#if s.user}
            <button class="remove" onclick={() => removeScore(s)} aria-label="Ta bort {s.title}">×</button>
          {/if}
        </li>
      {/each}
    </ul>
    <p class="note">Sjungen text finns bara på engelska. Svensk text visas i noterna.</p>
  </main>
{:else}
  <main class="reader">
    <nav class="top glass">
      <button class="icon" onclick={close} aria-label="Bibliotek"><Icon name="back" size={20} /> Bibliotek</button>
      <h2>{opened.title}</h2>
      <label class="choir"
        >Kör
        <select value={opened.choir} onchange={changeChoir}>
          <option value="auto">Auto</option>
          <option value="mixed">Blandad</option>
          <option value="male">Manskör</option>
          <option value="female">Damkör</option>
        </select>
      </label>
      {#if opened.pdf_url}
        <div class="seg" role="group" aria-label="Visning">
          <button class:on={view === 'pdf'} onclick={() => switchView('pdf')}>Original</button>
          <button class:on={view === 'notation'} onclick={() => switchView('notation')}>Notbild</button>
        </div>
      {/if}
      <button class="icon" class:active={appearanceOpen} onclick={() => (appearanceOpen = !appearanceOpen)} aria-label="Utseende"
        ><Icon name="appearance" /></button
      >
      <button class="icon" class:active={mixerOpen} onclick={() => (mixerOpen = !mixerOpen)}
        ><Icon name="sliders" /> {mixerOpen ? 'Dölj mixer' : 'Mixer'}</button
      >
      {#if appearanceOpen}
        <div class="popover glass"><Appearance /></div>
      {/if}
    </nav>

    <div class="body">
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      {#if view === 'pdf' && opened.pdf_url}
        <div class="score pdf">
          <PdfView
            url={scoreFileUrl(opened.pdf_url)}
            boxes={opened.measure_boxes}
            measure={playingMeasure}
            {follow}
            onseek={seekMeasure}
            loop={loopA !== null ? [loopA, loopB ?? loopA] : null}
          />
        </div>
      {:else}
      <div class="score" bind:this={scoreEl} onclick={tapScore}>
        {#if loading}<p class="loading">{loading}</p>{/if}
        {#if error}<p class="error">{error}</p>{/if}
        {@html svg}
        {#if engraved && engraved.pages > 1}
          <button class="pageturn left" onclick={(e) => { e.stopPropagation(); turn(-1) }} aria-label="Föregående sida"></button>
          <button class="pageturn right" onclick={(e) => { e.stopPropagation(); turn(1) }} aria-label="Nästa sida"></button>
        {/if}
      </div>
      {/if}

      {#if mixerOpen && opened.channels.length}
        <aside class="mixer glass">
          <div class="presets">
            <button onclick={() => preset('my_part')} disabled={mine === null}>Min stämma</button>
            <button onclick={() => preset('without_me')} disabled={mine === null}>Utan mig</button>
            <button onclick={() => preset('balanced')}>Balans</button>
          </div>
          {#each opened.channels as c, i (i)}
            <div class="strip {roleClass(c)}" class:mine={mine === i}>
              <button class="name" onclick={() => (mine = mine === i ? null : i)} title="Välj som min stämma">
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
                aria-label="Volym {c.label}"
              />
              <button class="m" class:on={strips[i].mute} onclick={() => setMute(i)}>M</button>
              <button class="s" class:on={strips[i].solo} onclick={() => setSolo(i)}>S</button>
            </div>
          {/each}
          <p class="hint">Tryck på ett namn för att välja din stämma.</p>
        </aside>
      {/if}
    </div>

    {#if !opened.channels.length}
      <footer class="transport glass silent">
        Bara PDF: inga noter att spela. Lägg till en MusicXML-fil med samma namn (från MuseScore eller en
        notskanner) så spelar stycket.
      </footer>
    {:else}
    <footer class="transport glass">
      <button class="icon" onclick={toStart} aria-label="Till början"><Icon name="start" /></button>
      <button class="play" onclick={toggle} aria-label={playing ? 'Paus' : 'Spela'}><Icon name={playing ? 'pause' : 'play'} size={24} /></button>
      <span class="time">{fmt(position)}</span>
      <input class="scrub" type="range" min="0" max={opened.duration} step="0.1" value={position} onchange={scrub} aria-label="Position" />
      <span class="time">{fmt(opened.duration)}</span>
      <label class="tempo"
        >Tempo {tempo}%
        <input type="range" min="25" max="200" step="5" value={tempo} oninput={setTempo} />
      </label>
      <button
        class="chip"
        class:on={loopPick !== 'off' || loopB !== null}
        onclick={toggleLoop}
        aria-label="Slinga A–B"
        ><Icon name="loop" size={18} />
        {#if loopPick === 'a'}Tryck första takten{:else if loopPick === 'b'}Tryck sista takten{:else if loopA !== null && loopB !== null}T. {loopA + 1}–{loopB + 1}{:else}Slinga{/if}</button
      >
      <button class="chip" class:on={metronome} onclick={toggleMetronome} aria-label="Metronom"
        ><Icon name="metronome" size={18} /> Klick</button
      >
      <button class="chip" class:on={countIn} onclick={toggleCountIn}>Inräkning</button>
      <label class="follow"><input type="checkbox" bind:checked={follow} /> Följ</label>
      {#if engraved && view === 'notation'}<span class="page">{page}/{engraved.pages}</span>{/if}
    </footer>
    {/if}
  </main>
{/if}
