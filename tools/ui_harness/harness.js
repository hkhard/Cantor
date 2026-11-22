// UI harness: the real Svelte UI (vite dev server) in WebKit at iPad mini
// size, with the Tauri backend replaced by an in-memory mock. Exercises the
// library (search, metadata, setlists), the reader (transpose, parts, page
// modes, pitch pipe, presets) and annotations, and saves screenshots.
const { webkit, devices } = require('playwright')
const fs = require('fs')
const path = require('path')

const DIR = __dirname
const xml = fs.readFileSync(path.join(DIR, 'Glimmande Nymf.musicxml'), 'utf8')
const pdf = fs.readFileSync(path.join(DIR, 'Glimmande Nymf.pdf'))
const boxes = JSON.parse(fs.readFileSync(path.join(DIR, 'Glimmande Nymf.measures.json'), 'utf8'))
const out = path.join(DIR, 'shots')
fs.mkdirSync(out, { recursive: true })

const W = Number(process.env.W || 744)
const H = Number(process.env.H || 1133)

;(async () => {
  const browser = await webkit.launch()
  const ctx = await browser.newContext({ viewport: { width: W, height: H }, deviceScaleFactor: 2, hasTouch: true })
  const page = await ctx.newPage()
  const errors = []
  page.on('pageerror', (e) => errors.push('pageerror: ' + e.message))
  page.on('console', (m) => {
    if (m.type() === 'error') errors.push('console: ' + m.text())
  })
  await page.route('http://cantor.localhost/**', (r) => r.fulfill({ body: pdf, contentType: 'application/pdf' }))
  await page.exposeBinding('__node', () => null)
  await page.addInitScript(
    ({ xml, boxes }) => {
      const calls = []
      window.__calls = calls
      const store = { metadata: {}, setlists: [], settings: {}, ann: null, pipe: [] }
      window.__store = store
      const scores = [
        { file: '0:Glimmande Nymf', title: 'Glimmande Nymf', playable: true, pdf: true, user: false },
        { file: '0:Christmas', title: 'Christmas Day', playable: true, pdf: false, user: false },
        { file: '1:Mine', title: 'Min egen', playable: false, pdf: true, user: true },
      ]
      const defSettings = () => ({ my_part: null, strips: [], tempo_percent: 100, view: 'pdf', choir: 'auto', presets: [], transpose: 0, hidden_parts: [] })
      let pos = 0
      let playing = false
      const handlers = {
        list_scores: () => scores.map((s) => ({ ...s, title: store.metadata[s.file]?.title || s.title })),
        library_data: () => ({ metadata: store.metadata, setlists: store.setlists }),
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
        open_score: ({ file, transpose }) => ({
          title: file.split(':')[1],
          musicxml: file === '1:Mine' ? '' : xml,
          channels:
            file === '1:Mine'
              ? []
              : [
                  { label: 'Tenor 1', role: 'Tenor', program: 56, instrument: 'Trumpet', pan: -0.4 },
                  { label: 'Tenor 2', role: 'Tenor', program: 60, instrument: 'French Horn', pan: -0.1 },
                  { label: 'Bas 1', role: 'Bass', program: 42, instrument: 'Cello', pan: 0.1 },
                  { label: 'Bas 2', role: 'Bass', program: 70, instrument: 'Bassoon', pan: 0.4 },
                ],
          measures: boxes.map((_, i) => [i * 2, i]),
          duration: boxes.length * 2,
          choir: 'auto',
          sample_rate: 48000,
          pdf_url: file === '0:Christmas' ? null : '0/Glimmande Nymf.pdf',
          measure_boxes: file === '0:Glimmande Nymf' ? boxes : null,
          _transpose: transpose,
        }),
        status: () => ({ playing, position: pos, duration: 60, tempo: 1 }),
        play: () => {
          playing = true
        },
        pause: () => {
          playing = false
        },
        seek: ({ seconds }) => {
          pos = seconds
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
          } else if (action === 'undo') d.items.pop()
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

  const shot = async (n) => page.screenshot({ path: path.join(out, `${n}.png`) })
  const step = async (name, fn) => {
    try {
      await fn()
      console.log('ok  ', name)
    } catch (e) {
      console.log('FAIL', name, '-', e.message.split('\n')[0])
      await shot('fail-' + name.replace(/\W+/g, '_'))
    }
  }

  await page.goto('http://localhost:5173/')
  await page.waitForSelector('text=Glimmande Nymf')
  await shot('01-library')

  await step('search filters', async () => {
    await page.fill('input[type=search]', 'christ')
    await page.waitForTimeout(400)
    const n = await page.locator('ul.scores > li').count()
    if (n !== 1) throw new Error(`expected 1 hit, got ${n}`)
    await page.fill('input[type=search]', '')
    await page.waitForTimeout(300)
  })

  await step('metadata editor saves', async () => {
    await page.locator('ul.scores > li', { hasText: 'Glimmande Nymf' }).locator('button.info').click()
    await page.waitForSelector('.metasheet')
    await shot('02-meta')
    const inputs = page.locator('.metasheet input')
    await inputs.nth(1).fill('Bellman')
    await inputs.nth(6).fill('vår, konsert')
    await page.locator('.metasheet .stars').first().locator('button').nth(3).click()
    await page.locator('.metasheet button.primary').click()
    await page.waitForSelector('.metasheet', { state: 'detached' })
    const m = await page.evaluate(() => window.__store.metadata['0:Glimmande Nymf'])
    if (m.composer !== 'Bellman' || m.tags.length !== 2 || m.rating !== 4) throw new Error(JSON.stringify(m))
    await page.waitForSelector('text=Bellman')
  })

  await step('search by composer', async () => {
    await page.fill('input[type=search]', 'bellman')
    await page.waitForTimeout(400)
    const n = await page.locator('ul.scores > li').count()
    if (n !== 1) throw new Error(`expected 1, got ${n}`)
    await page.fill('input[type=search]', '')
    await page.waitForTimeout(300)
  })

  await step('create setlist and add', async () => {
    await page.click('text=New setlist')
    await page.waitForSelector('.dlg input')
    await page.fill('.dlg input', 'Vårkonsert')
    await page.click('.dlg button.primary')
    await page.waitForSelector('h2.setname')
    await page.click('[role=tab] >> text=All scores')
    for (const t of ['Christmas Day', 'Glimmande Nymf']) {
      await page.locator('ul.scores > li', { hasText: t }).locator('button.add').click()
      await page.locator('.addmenu button', { hasText: 'Vårkonsert' }).click()
    }
    await page.click('[role=tab] >> text=Vårkonsert')
    await shot('03-setlist')
    const items = await page.locator('ol.setlist > li').count()
    if (items !== 2) throw new Error('items ' + items)
    await page.locator('ol.setlist > li').nth(1).locator('button.up').click()
    await page.waitForTimeout(200)
    const first = await page.evaluate(() => window.__store.setlists[0].items[0].score)
    if (first !== '0:Glimmande Nymf') throw new Error('reorder failed ' + first)
  })

  await step('open from setlist, next piece', async () => {
    await page.click('text=Play setlist')
    await page.waitForSelector('.top h2 >> text=Glimmande Nymf')
    await page.waitForSelector('.pdfhost canvas')
    await page.waitForTimeout(1500)
    await shot('04-reader-pdf')
    await page.click('.top button[aria-label="▶"]')
    await page.waitForSelector('.top h2 >> text=Christmas')
    await page.click('.top button[aria-label="◀"]')
    await page.waitForSelector('.top h2 >> text=Glimmande Nymf')
  })

  await step('pitch pipe all + one', async () => {
    await page.click('text=♪ Pitch')
    await page.click('.top button:has-text("Mixer")')
    await page.locator('.strip').nth(2).locator('button.pipe').click()
    const p = await page.evaluate(() => window.__store.pipe)
    if (JSON.stringify(p) !== JSON.stringify(['all', 2])) throw new Error(JSON.stringify(p))
    await shot('05-mixer')
  })

  await step('custom preset saved per score', async () => {
    await page.locator('.strip').nth(0).locator('button.m').click()
    await page.click('text=Save mix')
    await page.fill('.dlg input', 'Utan T1')
    await page.click('.dlg button.primary')
    await page.waitForTimeout(600)
    const s = await page.evaluate(() => window.__store.settings['0:Glimmande Nymf'])
    if (!s || s.presets[0]?.name !== 'Utan T1' || !s.presets[0].strips[0].mute) throw new Error(JSON.stringify(s))
    await page.click('.top button:has-text("Hide mixer")')
  })

  await step('page modes', async () => {
    for (const m of ['two', 'half', 'scroll', 'single']) {
      await page.click('.top button:has-text("•••")')
      await page.selectOption('.popover.more select >> nth=-1', m)
      await page.click('.top button:has-text("•••")')
      await page.waitForTimeout(900)
      await shot('06-mode-' + m)
      if (m === 'half') {
        await page.click('.pageturn.right')
        await page.waitForSelector('.halfover')
        await shot('06-mode-half-turned')
      }
    }
  })

  await step('annotate: pen stroke, stamp, undo, layer', async () => {
    await page.click('.top button:has-text("Annotate")')
    await page.waitForSelector('.annbar')
    const svg = page.locator('svg.annot').first()
    const b = await svg.boundingBox()
    await page.mouse.move(b.x + 100, b.y + 200)
    await page.mouse.down()
    for (let i = 1; i <= 10; i++) await page.mouse.move(b.x + 100 + i * 15, b.y + 200 + i * 3)
    await page.mouse.up()
    await page.click('.annbar button:has-text("Stamp")')
    await page.click('.annbar .stamps button:has-text("mf")')
    await page.mouse.click(b.x + 300, b.y + 300)
    await page.waitForTimeout(300)
    await shot('07-annotated')
    let d = await page.evaluate(() => window.__store.ann)
    if (d.items.length !== 2 || !('Ink' in d.items[0].item) || d.items[1].item.Stamp.kind !== 'Mf') throw new Error(JSON.stringify(d.items.map((i) => Object.keys(i.item))))
    if (d.items[0].item.Ink.points.length < 5) throw new Error('few points')
    await page.click('.annbar button[aria-label="Undo"]')
    await page.waitForTimeout(200)
    d = await page.evaluate(() => window.__store.ann)
    if (d.items.length !== 1) throw new Error('undo')
    const lines = await page.locator('svg.annot line').count()
    if (lines < 5) throw new Error('ink not drawn: ' + lines)
    await page.click('.annbar button:has-text("Layer")')
    await page.click('text=New layer')
    await page.fill('.dlg input', 'Dirigent')
    await page.click('.dlg button.primary')
    await page.waitForTimeout(300)
    d = await page.evaluate(() => window.__store.ann)
    if (d.layers.length !== 2) throw new Error('layer')
    await page.locator('.layers input[type=checkbox]').first().uncheck()
    await page.waitForTimeout(300)
    const after = await page.locator('svg.annot line').count()
    if (after !== 0) throw new Error('hidden layer still drawn ' + after)
    await page.click('.top button:has-text("Done")')
  })

  await step('transpose + hide part (notation)', async () => {
    await page.click('.seg button:has-text("Notation")')
    await page.waitForSelector('.score svg', { timeout: 20000 })
    await page.click('.top button:has-text("•••")')
    await page.click('.popover.more button[aria-label="+1"]')
    await page.waitForTimeout(1500)
    const tr = await page.evaluate(() => window.__calls.filter((c) => c[0] === 'open_score').pop()[1].transpose)
    if (tr !== 1) throw new Error('transpose arg ' + tr)
    await page.waitForSelector('.score svg', { timeout: 20000 })
    if (!(await page.locator('.popover.more').count())) await page.click('.top button:has-text("•••")')
    const before = await page.locator('.score svg g.staff').count()
    await page.locator('.popover.more .check input').first().uncheck()
    await page.waitForTimeout(2500)
    await page.click('.top button:has-text("•••")')
    const after = await page.locator('.score svg g.staff').count()
    await shot('08-notation-hidden')
    if (!(after < before)) throw new Error(`staves ${before} -> ${after}`)
    const s = await page.evaluate(() => window.__store.settings['0:Glimmande Nymf'])
    if (s.transpose !== 1 || s.hidden_parts[0] !== 0) throw new Error(JSON.stringify(s))
  })

  await step('settings restored on reopen', async () => {
    await page.click('.top button[aria-label="Library"]')
    await page.waitForSelector('text=New setlist')
    await page.click('[role=tab] >> text=All scores')
    await page.locator('ul.scores > li', { hasText: 'Glimmande Nymf' }).locator('button.open').click()
    await page.waitForSelector('.top h2')
    await page.waitForTimeout(1500)
    const last = await page.evaluate(() => window.__calls.filter((c) => c[0] === 'open_score').pop()[1])
    if (last.transpose !== 1) throw new Error('transpose not restored')
    const view = await page.locator('.seg button.on').first().textContent()
    if (!/Notation/.test(view)) throw new Error('view not restored: ' + view)
  })

  await step('performance mode', async () => {
    await page.click('.top button:has-text("•••")')
    await page.click('.popover.more button:has-text("Perform")')
    await page.waitForSelector('.perfexit')
    await shot('09-performance')
    await page.click('.perfexit')
  })

  await step('swedish', async () => {
    await page.click('.top button[aria-label="Library"]')
    await page.click('header button[aria-label="About Cantor"]')
    await page.selectOption('.about select', 'sv')
    await page.waitForSelector('text=Licenser')
    await shot('10-about-sv')
    await page.click('.about button.primary')
    await page.waitForSelector('text=Alla noter')
    await shot('11-library-sv')
  })

  console.log(errors.length ? 'ERRORS:\n' + errors.join('\n') : 'no page errors')
  await browser.close()
})()
