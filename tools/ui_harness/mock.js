// Shared harness setup: the Tauri backend mocked in memory, the PDF served
// for the cantor:// scheme. Used by harness.js (features) and a11y.js.
const fs = require('fs')
const path = require('path')

exports.setup = async function setup(page, dir) {
  const xml = fs.readFileSync(path.join(dir, 'score.musicxml'), 'utf8')
  const pdf = fs.readFileSync(path.join(dir, 'score.pdf'))
  const boxes = JSON.parse(fs.readFileSync(path.join(dir, 'score.measures.json'), 'utf8'))
  await page.route('http://cantor.localhost/**', (r) => r.fulfill({ body: pdf, contentType: 'application/pdf' }))
  // DATA-01 / NFR-05 / NFR-07: the app must work with no network and must not
  // call out. Every request that is not the local dev server or the cantor://
  // scheme is blocked and recorded in page.__external.
  page.__external = []
  await page.route(/^(?!http:\/\/(localhost|127\.0\.0\.1|cantor\.localhost)[:/]).*/, (r) => {
    const u = r.request().url()
    if (u.startsWith('data:') || u.startsWith('blob:')) return r.continue()
    page.__external.push(u)
    return r.abort('internetdisconnected')
  })
  await page.exposeBinding('__node', () => null)
  await page.addInitScript(
    ({ xml, boxes }) => {
      window.__xml = xml
      const calls = []
      window.__calls = calls
      const store = { metadata: {}, setlists: [], settings: {}, ann: null, pipe: [], scoreOrder: [], setlistOrder: [], home: { group: 'setlists', tab: '' } }
      // Survives page reloads within one harness run (like library.json).
      try {
        const kept = JSON.parse(sessionStorage.getItem('__cantorStore') || 'null')
        if (kept) Object.assign(store, kept)
      } catch {}
      // Everything except the transient annotation document survives.
      addEventListener('beforeunload', () => {
        const { ann, ...keep } = store
        void ann
        sessionStorage.setItem('__cantorStore', JSON.stringify(keep))
      })
      window.__store = store
      const scores = [
        { file: '0:Lacrymosa', title: 'Lacrymosa', playable: true, pdf: true, user: false },
        { file: '0:Christmas', title: 'Christmas Day', playable: true, pdf: false, user: false },
        { file: '1:Mine', title: 'Min egen', playable: false, pdf: true, user: true },
        { file: '1:Scanned', title: 'Skannad', playable: true, pdf: false, user: true },
      ]
      // Test hook: add scores (e.g. a MusicXML and a PDF that arrived apart).
      window.__addScores = (list) => scores.push(...list)
      window.__removeScore = (file) => {
        const i = scores.findIndex((x) => x.file === file)
        if (i >= 0) scores.splice(i, 1)
      }
      const defSettings = () => ({ my_part: null, strips: [], tempo_percent: 100, view: 'pdf', choir: 'auto', presets: [], transpose: 0, hidden_parts: [] })
      let pos = 0
      let playing = false
      const handlers = {
        list_scores: () => {
          const pos = (id) => {
            const i = store.scoreOrder.indexOf(id)
            return i < 0 ? 1e9 : i
          }
          return scores
            .map((s) => ({ ...s, title: store.metadata[s.file]?.title || s.title }))
            .sort((a, b) => pos(a.file) - pos(b.file)) // unlisted keep the fixture order
        },
        library_data: () => {
          const pos = (id) => {
            const i = store.setlistOrder.indexOf(id)
            return i < 0 ? 1e9 : i
          }
          const setlists = store.setlists.map((s, i) => [s, i]).sort((a, b) => pos(a[0].id) - pos(b[0].id) || a[1] - b[1]).map((x) => x[0])
          return { metadata: store.metadata, setlists, home: store.home }
        },
        pdf_url_of: ({ file }) => (scores.find((s) => s.file === file)?.pdf ? '0/x.pdf' : null),
        read_title_page: ({ png }) => {
          store.ocrBytes = png.length
          store.ocrPng = png
          return { title: 'Vårvindar friska', subtitle: 'för blandad kör', composer: 'Svensk folkmelodi', arranger: 'Hugo Alfvén', lyricist: 'Julia Nyberg', choir: 'SATB' }
        },
        pair_suggestions: () => (store.pairs ?? []).filter((p) => !(store.dismissed ?? []).includes(p.join('|'))),
        combine_scores: ({ score, pdf, dismiss }) => {
          if (dismiss) {
            store.dismissed = [...(store.dismissed ?? []), score + '|' + pdf]
            return
          }
          const s = scores.find((x) => x.file === score)
          const i = scores.findIndex((x) => x.file === pdf)
          if (!s || i < 0) throw 'not found'
          s.pdf = true
          scores.splice(i, 1)
          store.pairs = (store.pairs ?? []).filter((p) => p[1] !== pdf)
          store.combined = [score, pdf]
        },
        set_home_view: ({ view }) => {
          store.home = view
        },
        set_order: ({ kind, ids }) => {
          if (kind === 'scores') store.scoreOrder = ids
          else store.setlistOrder = ids
        },
        set_metadata: ({ id, metadata }) => {
          store.metadata[id] = metadata
        },
        search_scores: ({ query }) => {
          const q = query.toLowerCase()
          return scores
            .filter((s) => {
              const m = store.metadata[s.file]
              return [s.title, m?.composer, ...(m?.tags ?? [])].some((x) => x && x.toLowerCase().includes(q))
            })
            .map((s) => s.file)
        },
        save_setlist: ({ setlist, name }) => {
          if (setlist) {
            store.setlists = store.setlists.map((s) => (s.id === setlist.id ? setlist : s))
            return setlist.id
          }
          const id = 'sl' + (store.setlists.length + 1)
          store.setlists.push({ id, name, items: [] })
          return id
        },
        delete_setlist: ({ id }) => {
          store.setlists = store.setlists.filter((s) => s.id !== id)
        },
        score_settings: ({ id }) => store.settings[id] ?? defSettings(),
        save_score_settings: ({ id, settings }) => {
          store.settings[id] = settings
        },
        instruments: () => Array.from({ length: 128 }, (_, p) => [p, ['Acoustic Grand Piano', 'Bright Acoustic Piano'][p] ?? (p === 19 ? 'Church Organ' : p === 52 ? 'Choir Aahs' : `GM ${p}`)]),
        voice_sets: () => ({
          sets: [
            { id: 'classic', name_sv: 'Klassisk', name_en: 'Classic (distinct colours)', choir: '', roles: {} },
            { id: 'choir-aahs', name_sv: 'Kör (aah)', name_en: 'Choir aahs', choir: '', roles: { tenor: [52], bass: [52] } },
            { id: 'organ', name_sv: 'Orgel', name_en: 'Organ', choir: '', roles: { tenor: [19], bass: [20] } },
            ...(store.ownSets ?? []),
          ],
          defaults: { mixed: 'classic', male: store.defaultMale ?? 'classic', female: 'classic' },
        }),
        apply_voice_set: ({ id, file, defaultFor }) => {
          if (defaultFor === 'male') store.defaultMale = id
          if (file) {
            const s = (store.settings[file] ??= defSettings())
            s.voice_set = id
            s.programs = []
          }
        },
        save_voice_set: ({ set }) => {
          const id = 'own-' + ((store.ownSets ?? []).length + 1)
          store.ownSets = [...(store.ownSets ?? []), { ...set, id }]
          return id
        },
        remove_voice_set: ({ id }) => {
          store.ownSets = (store.ownSets ?? []).filter((v) => v.id !== id)
        },
        set_instrument: ({ file, index, program }) => {
          const s = (store.settings[file] ??= defSettings())
          s.programs = s.programs ?? []
          s.programs[index] = program
          store.instrumentCalls = [...(store.instrumentCalls ?? []), [file, index, program]]
        },
        open_score: ({ file, transpose }) => ({
          title: file.split(':')[1],
          musicxml: file === '1:Mine' ? '' : xml,
          channels:
            file === '1:Mine'
              ? store.track
                ? [{ label: 'Backing track', role: 'Track', program: 0, instrument: 'M4A', pan: 0 }]
                : []
              : [
                  { label: 'Tenor 1', role: 'Tenor', program: 56, instrument: 'Trumpet', pan: -0.4 },
                  { label: 'Tenor 2', role: 'Tenor', program: 60, instrument: 'French Horn', pan: -0.1 },
                  { label: 'Bas 1', role: 'Bass', program: 42, instrument: 'Cello', pan: 0.1 },
                  { label: 'Bas 2', role: 'Bass', program: 70, instrument: 'Bassoon', pan: 0.4 },
                ].map((c, i) => {
                  // Same order as the backend: own choice, else the score's voice set.
                  const s = store.settings[file]
                  const own = s?.programs?.[i]
                  const set = s?.voice_set === 'choir-aahs' ? 52 : s?.voice_set === 'organ' ? 19 : null
                  const p = own ?? set ?? c.program
                  return { ...c, program: p, instrument: p === 52 ? 'Choir Aahs' : p === 19 ? 'Church Organ' : c.instrument }
                }),
          measures: boxes.map((_, i) => [i * 2, i]),
          duration: boxes.length * 2,
          choir: 'auto',
          voice_choir: 'male',
          sample_rate: 48000,
          pdf_url: file === '0:Christmas' ? null : '0/score.pdf',
          measure_boxes: file === '0:Lacrymosa' ? boxes : null,
          track: store.track && file === '1:Mine' ? { offset_sec: store.track.offset_sec, markers: store.track.markers, duration_sec: 30 } : null,
          _transpose: transpose,
        }),
        status: () => {
          // PERF-04: markers fire as the mock clock passes them.
          if (playing) pos += 0.25
          const turns = []
          const t = store.track
          if (t && playing) {
            for (const [at, pg] of t.markers) if (!t.fired.has(at) && pos + t.offset_sec >= at) {
              t.fired.add(at)
              turns.push(pg)
            }
          }
          return { playing, position: pos, duration: 60, tempo: 1, track_position: t ? pos + t.offset_sec : null, turns }
        },
        attach_track: () => {
          store.track = { offset_sec: 0, markers: [], duration_sec: 30, fired: new Set() }
        },
        remove_track: () => {
          store.track = null
        },
        set_track_info: ({ offsetSec, markers }) => {
          store.track = { ...store.track, offset_sec: offsetSec, markers, fired: new Set() }
        },
        play: () => {
          playing = true
        },
        pause: () => {
          playing = false
        },
        seek: ({ seconds }) => {
          pos = seconds
        },
        backup_library: () => {
          store.backups = (store.backups ?? 0) + 1
          return 'Cantor 2026-11-22.cantorbackup'
        },
        pending_backups: () => store.pending ?? [],
        restore_pending: ({ name, discard }) => {
          store.pending = (store.pending ?? []).filter((n) => n !== name)
          store.restored = discard ? store.restored : name
          return discard ? 0 : 7
        },
        export_item: ({ kind, id }) => {
          store.exports = [...(store.exports ?? []), [kind, id]]
          return kind === 'setlist' ? 'Vårkonsert.cantorset' : kind === '4ss' ? 'Vårkonsert.4ss' : 'x.' + kind
        },
        correct_note: (a) => {
          store.corrections = [...(store.corrections ?? []), a]
        },
        sync_folder_name: () => store.syncFolder ?? null,
        choose_sync_folder: () => {
          store.syncFolder = 'Cantor (iCloud)'
          return store.syncFolder
        },
        forget_sync_folder: () => {
          store.syncFolder = null
        },
        sync_now: () => {
          store.syncs = (store.syncs ?? 0) + 1
          return { folder: store.syncFolder ?? null, sent: store.syncFolder ? 2 : 0, received: store.syncFolder ? 1 : 0, changed: 0, pending: 0 }
        },
        pitch_pipe: ({ channel }) => {
          store.pipe.push(channel ?? 'all')
        },
        annotations_open: () => {
          store.ann = store.ann ?? { version: 1, layers: [{ name: 'Default', visible: true }], items: [], next_id: 1 }
          return store.ann
        },
        annotations_edit: ({ action, command }) => {
          const d = store.ann
          if (action === 'apply') {
            if (command.op === 'Add') d.items.push({ id: d.next_id++, layer: command.layer, anchor: command.anchor, item: command.item })
            if (command.op === 'AddLayer') d.layers.push({ name: command.name, visible: true })
            if (command.op === 'SetLayerVisible') d.layers[command.layer].visible = command.visible
            if (command.op === 'Erase') d.items = d.items.filter((a) => !('Ink' in a.item))
            if (command.op === 'RemoveMany') {
              d._removed = d._removed ?? []
              d._removed.push(d.items.filter((a) => command.ids.includes(a.id)))
              d.items = d.items.filter((a) => !command.ids.includes(a.id))
            }
          } else if (action === 'undo') {
            if (d._removed?.length) d.items.push(...d._removed.pop())
            else d.items.pop()
          }
          return JSON.parse(JSON.stringify(d))
        },
      }
      window.__TAURI_INTERNALS__ = {
        invoke: async (cmd, args) => {
          calls.push([cmd, args])
          const h = handlers[cmd]
          return h ? JSON.parse(JSON.stringify(h(args ?? {}) ?? null)) : null
        },
        transformCallback: (cb) => {
          const id = Math.random()
          window['_' + id] = cb
          return id
        },
        unregisterCallback: () => {},
        convertFileSrc: (p) => p,
        metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      }
      window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} }
    },
    { xml, boxes },
  )
  // Force the non-WebKit URL branch in scoreFileUrl (served by the route above).
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'userAgent', { get: () => 'Mozilla/5.0 (Linux) Harness' })
  })

}
