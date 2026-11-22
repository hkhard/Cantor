// UI harness: the real Svelte UI (vite dev server) in WebKit at iPad mini
// size, with the Tauri backend replaced by an in-memory mock. Exercises the
// library (search, metadata, setlists), the reader (transpose, parts, page
// modes, pitch pipe, presets) and annotations, and saves screenshots.
const { webkit, devices } = require('playwright')
const fs = require('fs')
const path = require('path')

const DIR = __dirname
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
  await require('./mock.js').setup(page, DIR)
  const shot = async (n) => page.screenshot({ path: path.join(out, `${n}.png`) })
  let failed = 0
  const step = async (name, fn) => {
    try {
      await fn()
      console.log('ok  ', name)
    } catch (e) {
      failed++
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
    if (!(await page.locator('.layers').count())) await page.click('.annbar button:has-text("Layer")')
    await page.locator('.layers input[type=checkbox]').first().uncheck()
    await page.waitForTimeout(300)
    const after = await page.locator('svg.annot line').count()
    if (after !== 0) throw new Error('hidden layer still drawn ' + after)
    await page.locator('.layers input[type=checkbox]').first().check()
    if (await page.locator('.layers').count()) await page.click('.annbar button:has-text("Layer")')
    await page.click('.top button:has-text("Done")')
  })

  await step('ANN-04 ink on PDF bar is bar-anchored and shows in notation', async () => {
    // The pen stroke drawn on the PDF earlier landed on a bar (measure map).
    let d = await page.evaluate(() => window.__store.ann)
    const ink = d.items.find((a) => 'Ink' in a.item)
    if (!ink || !('Measure' in ink.anchor)) throw new Error('PDF stroke not bar-anchored: ' + JSON.stringify(ink?.anchor))
  })

  await step('ANN-04 draw on notation, survives reflow', async () => {
    await page.click('.seg button:has-text("Notation")')
    await page.waitForSelector('.score svg g.measure', { timeout: 20000 })
    await page.waitForTimeout(800)
    await page.click('.top button:has-text("Annotate")')
    await page.waitForSelector('.annbar')
    await page.click('.annbar button:has-text("Pen")')
    // Draw across the middle of the 3rd bar on the page.
    const bar = page.locator('.score svg g.measure').nth(2)
    const bb = await bar.boundingBox()
    const y = bb.y + bb.height * 0.5
    await page.mouse.move(bb.x + bb.width * 0.2, y)
    await page.mouse.down()
    for (let i = 1; i <= 8; i++) await page.mouse.move(bb.x + bb.width * (0.2 + i * 0.075), y + (i % 2) * 4)
    await page.mouse.up()
    await page.waitForTimeout(300)
    let d = await page.evaluate(() => window.__store.ann)
    const it = d.items[d.items.length - 1]
    if (!('Measure' in it.anchor)) throw new Error('not bar-anchored ' + JSON.stringify(it.anchor))
    const m = it.anchor.Measure.measure
    const check = async (label) => {
      // Stroke bbox vs. its bar's bbox on screen.
      const res = await page.evaluate((id) => {
        const lines = document.querySelectorAll(`.score svg.annot g[data-ann="${id}"] line`)
        if (!lines.length) return { err: 'no ink drawn' }
        let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity
        for (const l of lines) {
          const r = l.getBoundingClientRect()
          x0 = Math.min(x0, r.left); x1 = Math.max(x1, r.right); y0 = Math.min(y0, r.top); y1 = Math.max(y1, r.bottom)
        }
        const bars = document.querySelectorAll('.score svg g.measure')
        return { x0, x1, y0, y1, bars: [...bars].map((b) => { const r = b.getBoundingClientRect(); return [r.left, r.right, r.top, r.bottom] }) }
      }, it.id)
      if (res.err) throw new Error(label + ': ' + res.err)
      const cx = (res.x0 + res.x1) / 2, cy = (res.y0 + res.y1) / 2
      const inBar = res.bars.findIndex(([l, r, t, b]) => cx >= l && cx <= r && cy >= t && cy <= b)
      return { cx, cy, inBar, w: res.x1 - res.x0 }
    }
    const before = await check('before')
    await shot('12-notation-ink')
    // Reflow: open the mixer (narrower pane), notation is engraved again.
    await page.click('.top button:has-text("Mixer")')
    await page.waitForTimeout(2500)
    const after = await check('after')
    await shot('13-notation-ink-reflowed')
    const barIdAfter = await page.evaluate((m) => {
      const bars = [...document.querySelectorAll('.score svg g.measure')]
      return bars.map((b) => b.getAttribute('data-id'))
    }, m)
    if (after.inBar < 0) throw new Error('stroke left its bar after reflow ' + JSON.stringify(after))
    if (before.inBar !== 2) throw new Error('stroke not on bar 3 before: ' + before.inBar)
    console.log('     stroke centre in bar', before.inBar, '->', after.inBar, 'width', Math.round(before.w), '->', Math.round(after.w))
    await page.click('.top button:has-text("Hide mixer")')
    await page.waitForTimeout(1500)
    // Erase it on the notation.
    await page.click('.annbar button:has-text("Eraser")')
    const bb2 = await page.locator('.score svg g.measure').nth(2).boundingBox()
    await page.mouse.move(bb2.x + bb2.width * 0.2, bb2.y + bb2.height * 0.5)
    await page.mouse.down()
    for (let i = 1; i <= 10; i++) await page.mouse.move(bb2.x + bb2.width * (0.2 + i * 0.07), bb2.y + bb2.height * 0.5)
    await page.mouse.up()
    await page.waitForTimeout(300)
    d = await page.evaluate(() => window.__store.ann)
    if (d.items.some((a) => a.id === it.id)) throw new Error('eraser missed bar-anchored ink')
    await page.click('.annbar button[aria-label="Undo"]')
    await page.waitForTimeout(300)
    d = await page.evaluate(() => window.__store.ann)
    if (!d.items.some((a) => a.id === it.id)) throw new Error('undo of erase failed')
    // The same stroke on the original page (measure map).
    await page.click('.top button:has-text("Done")')
    await page.click('.seg button:has-text("Original")')
    await page.waitForTimeout(1500)
    await shot('14-notation-ink-on-pdf')
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
  process.exit(failed || errors.length ? 1 : 0)
})()
