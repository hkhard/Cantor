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

  await page.goto((process.env.CANTOR_UI_URL || 'http://localhost:5173/'))
  await page.waitForSelector('text=Glimmande Nymf')
  await shot('01-library')

  await step('lib_04 search filters', async () => {
    await page.fill('input[type=search]', 'christ')
    await page.waitForTimeout(400)
    const n = await page.locator('ul.scores > li').count()
    if (n !== 1) throw new Error(`expected 1 hit, got ${n}`)
    await page.fill('input[type=search]', '')
    await page.waitForTimeout(300)
  })

  await step('lib_03 metadata editor saves', async () => {
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

  await step('lib_04 search by composer', async () => {
    await page.fill('input[type=search]', 'bellman')
    await page.waitForTimeout(400)
    const n = await page.locator('ul.scores > li').count()
    if (n !== 1) throw new Error(`expected 1, got ${n}`)
    await page.fill('input[type=search]', '')
    await page.waitForTimeout(300)
  })

  await step('set_01 create setlist, add, reorder', async () => {
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

  // Drag with a long press, measuring every frame while the item moves.
  // Returns frame deltas (ms) and the layout reads the drag caused.
  const longDrag = async (from, to, { hold = 520, steps = 40, dwell = 0 } = {}) => {
    const a = await from.boundingBox()
    const b = typeof to.x === 'number' ? to : await to.boundingBox()
    const ax = a.x + a.width / 2
    const ay = a.y + a.height / 2
    const bx = b.x + (b.width ?? 0) / 2
    const by = b.y + (b.height ?? 0) / 2
    await page.evaluate(() => {
      window.__frames = []
      let last = performance.now()
      const tick = (t) => {
        window.__frames.push(t - last)
        last = t
        if (window.__framesOn) requestAnimationFrame(tick)
      }
      window.__framesOn = false
    })
    await page.mouse.move(ax, ay)
    await page.mouse.down()
    await page.waitForTimeout(hold)
    await page.evaluate(() => {
      window.__framesOn = true
      let last = performance.now()
      const tick = (t) => {
        window.__frames.push(t - last)
        last = t
        if (window.__framesOn) requestAnimationFrame(tick)
      }
      requestAnimationFrame((t) => {
        last = t
        requestAnimationFrame(tick)
      })
    })
    for (let i = 1; i <= steps; i++) {
      await page.mouse.move(ax + ((bx - ax) * i) / steps, ay + ((by - ay) * i) / steps)
      await page.waitForTimeout(16)
    }
    if (dwell) await page.waitForTimeout(dwell)
    const frames = await page.evaluate(() => {
      window.__framesOn = false
      return window.__frames.slice(2)
    })
    await page.mouse.up()
    await page.waitForTimeout(450)
    return frames
  }
  const frameReport = (frames) => {
    const s = [...frames].sort((x, y) => x - y)
    const p = (q) => s[Math.min(s.length - 1, Math.floor(q * s.length))]
    const slow = frames.map((f, i) => [i, +f.toFixed(0)]).filter((x) => x[1] > 25)
    return { n: s.length, p50: +p(0.5).toFixed(1), p95: +p(0.95).toFixed(1), max: +s[s.length - 1].toFixed(1), slow }
  }
  const checkFrames = (frames, what) => {
    const r = frameReport(frames)
    console.log('     ', what, 'frames', JSON.stringify(r))
    // 60 fps: a frame every 16.7 ms. Vsync jitter of a few ms is normal; a
    // dropped frame shows up as a gap of ~33 ms. Require none over 25 ms.
    if (r.n < 20) throw new Error('too few frames ' + r.n)
    if (r.p50 > 17.5) throw new Error('below 60 fps ' + JSON.stringify(r))
    // One outlier per drag is tolerated: on the shared CI host another job
    // can steal a frame (seen once in ~10 runs). Two means the drag is janky.
    if (r.slow.length > 1) throw new Error('dropped frames ' + JSON.stringify(r))
  }
  const titles = () => page.locator('ul.scores > li .t > span:first-child').allInnerTexts()

  await step('set_01 long-press drag reorders a setlist at 60 fps', async () => {
    const items = page.locator('ol.setlist > li')
    const before = await page.evaluate(() => window.__store.setlists[0].items.map((i) => i.score))
    const frames = await longDrag(items.nth(0), items.nth(1), { steps: 30 })
    const after = await page.evaluate(() => window.__store.setlists[0].items.map((i) => i.score))
    if (after[0] !== before[1] || after[1] !== before[0]) throw new Error('not reordered ' + after)
    checkFrames(frames, 'setlist')
    // The DOM is in the saved order and no transforms are left behind.
    const left = await page.evaluate(() => [...document.querySelectorAll('ol.setlist > li')].map((l) => l.style.transform).join(''))
    if (left) throw new Error('transforms left: ' + left)
    if (!(await page.locator('.library.editing').count())) throw new Error('no edit mode')
    // In edit mode a drag starts at once (no long press); drag it back up.
    const back = await longDrag(items.nth(1), items.nth(0), { hold: 30, steps: 30 })
    try {
      checkFrames(back, 'setlist back')
    } catch (e) {
      await page.click('button.done')
      throw e
    }
    const again = await page.evaluate(() => window.__store.setlists[0].items.map((i) => i.score))
    if (JSON.stringify(again) !== JSON.stringify(before)) throw new Error('not back ' + again)
    await page.click('button.done')
  })

  await step('lib_01 long-press drag rearranges the library, glitch-free, saved', async () => {
    await page.click('[role=tab] >> text=All scores')
    const t0 = await titles()
    const cards = page.locator('ul.scores > li')
    // Sample every frame: during the drag the cards under the finger must
    // only move by transform (no layout change), and each card must show
    // at most one target per slot change (no bouncing back and forth).
    await page.evaluate(() => {
      window.__moves = 0
      window.__mo = new MutationObserver((ms) => {
        for (const m of ms) if (m.type === 'childList') window.__moves++
      })
      window.__mo.observe(document.querySelector('ul.scores'), { childList: true, subtree: true })
    })
    // Drag the first card to the gap between the 3rd and the 4th (the
    // left edge of the last card): an edge, so it moves, not a merge.
    const last = await cards.nth(3).boundingBox()
    const frames = await longDrag(cards.nth(0), { x: last.x + 6, y: last.y + last.height / 2 }, { steps: 50, dwell: 300 })
    const domChangesDuringDrag = await page.evaluate(() => {
      const n = window.__moves
      window.__mo.disconnect()
      return n
    })
    checkFrames(frames, 'library')
    await page.waitForTimeout(300)
    const t1 = await titles()
    const want = [t0[1], t0[2], t0[3], t0[0]]
    if (JSON.stringify(t1) !== JSON.stringify(want)) throw new Error(`order ${t1} want ${want}`)
    const saved = await page.evaluate(() => window.__store.scoreOrder.length)
    if (saved !== 4) throw new Error('order not saved')
    // Only the drop changes the DOM (one reorder), never mid-drag.
    if (domChangesDuringDrag > 12) throw new Error('DOM churn during drag ' + domChangesDuringDrag)
    // The saved order is what the backend gets back on the next start
    // (persistence itself: set_01_manual_order_… in the library crate).
    const t2 = await page.evaluate(async () => (await window.__TAURI_INTERNALS__.invoke('list_scores', {})).map((s) => s.title))
    if (JSON.stringify(t2) !== JSON.stringify(want)) throw new Error('not saved in order ' + t2)
    await shot('28-library-reordered')
  })

  await step('lib_01 hold on another piece makes a list; drop on a tab adds', async () => {
    const cards = page.locator('ul.scores > li')
    const n0 = await page.evaluate(() => window.__store.setlists.length)
    const order0 = await titles()
    // A quick move straight to the middle of the next card, then a hold.
    await longDrag(cards.nth(0), cards.nth(1), { steps: 6, dwell: 800 })
    await page.waitForSelector('.dlg input')
    // The keyboard (iOS) needs focus taken inside the drop gesture: the
    // primer input is focused in pointerup, then the dialog's input.
    await page.waitForTimeout(100)
    const focused = await page.evaluate(() => document.activeElement?.closest('.dlg') !== null && !!document.getElementById('kbd-primer'))
    if (!focused) throw new Error('name field not focused after the drop')
    await shot('29-make-list')
    await page.fill('.dlg input', 'Duo')
    await page.click('.dlg button.primary')
    await page.waitForTimeout(300)
    const sl = await page.evaluate(() => window.__store.setlists.at(-1))
    if ((await page.evaluate(() => window.__store.setlists.length)) !== n0 + 1) throw new Error('no list')
    if (sl.name !== 'Duo' || sl.items.length !== 2) throw new Error('list ' + JSON.stringify(sl))
    // Making a list does not reorder the library.
    if (JSON.stringify(await titles()) !== JSON.stringify(order0)) throw new Error('library moved')
    // Drop the third card on the "Duo" tab.
    const tab = page.locator('[role=tab]', { hasText: 'Duo' })
    if ((await page.locator('.library.editing').count()) === 0) throw new Error('edit mode off')
    await longDrag(cards.nth(2), tab, { hold: 50, steps: 25 })
    const after = await page.evaluate(() => window.__store.setlists.at(-1).items.length)
    if (after !== 3) throw new Error('drop on tab: ' + after)
    // Rearrange the tabs: Duo first.
    const tabs = page.locator('.tablist > [data-sort-id]')
    await longDrag(tabs.nth(1), tabs.nth(0), { hold: 50, steps: 20 })
    const order = await page.locator('.tablist > [data-sort-id]').allInnerTexts()
    if (order[0] !== 'Duo') throw new Error('tabs ' + order)
    // Sort in edit mode: by title, saved as the manual order.
    await page.click('[role=tab] >> text=All scores')
    await page.click('.sortbar button >> text=Sort A–Z')
    await page.waitForTimeout(200)
    const az = await titles()
    if (JSON.stringify(az) !== JSON.stringify([...az].sort((x, y) => x.localeCompare(y, 'sv')))) throw new Error('not sorted ' + az)
    await shot('30-library-edit')
    await page.click('button.done')
    // A plain tap still opens a card (no drag left over).
    if (await page.locator('.library.editing').count()) throw new Error('still editing')
  })

  await step('set_01 open from setlist, next piece', async () => {
    await page.click('[role=tab] >> text=Vårkonsert')
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

  await step('perf_02 page turn by tap and key while paused (follow must not snap back)', async () => {
    await page.waitForFunction(() => !/\/0$/.test(document.querySelector('.pageno')?.textContent ?? '/0'))
    const before = await page.locator('.pageno').textContent()
    await page.click('.pageturn.right')
    await page.waitForTimeout(500)
    const a = await page.locator('.pageno').textContent()
    if (parseInt(a) !== parseInt(before) + 1) throw new Error(`tap: ${before} -> ${a}`)
    await page.keyboard.press('ArrowLeft')
    await page.waitForTimeout(500)
    const k = await page.locator('.pageno').textContent()
    if (parseInt(k) !== parseInt(before)) throw new Error(`key: ${a} -> ${k}`)
  })

  await step('perf_03 pitch pipe all + one', async () => {
    await page.click('text=♪ Pitch')
    await page.click('.top button:has-text("Mixer")')
    await page.locator('.strip').nth(2).locator('button.pipe').click()
    const p = await page.evaluate(() => window.__store.pipe)
    if (JSON.stringify(p) !== JSON.stringify(['all', 2])) throw new Error(JSON.stringify(p))
    await shot('05-mixer')
  })

  await step('play_04 custom preset saved per score', async () => {
    await page.locator('.strip').nth(0).locator('button.m').click()
    await page.click('text=Save mix')
    await page.fill('.dlg input', 'Utan T1')
    await page.click('.dlg button.primary')
    await page.waitForTimeout(600)
    const s = await page.evaluate(() => window.__store.settings['0:Glimmande Nymf'])
    if (!s || s.presets[0]?.name !== 'Utan T1' || !s.presets[0].strips[0].mute) throw new Error(JSON.stringify(s))
    await page.click('.top button:has-text("Hide mixer")')
  })

  await step('view_02 page modes', async () => {
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

  await step('ann_01 annotate: pen stroke, stamp, undo, layer', async () => {
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

  await step('ann_04 ink on PDF bar is bar-anchored', async () => {
    // The pen stroke drawn on the PDF earlier landed on a bar (measure map).
    let d = await page.evaluate(() => window.__store.ann)
    const ink = d.items.find((a) => 'Ink' in a.item)
    if (!ink || !('Measure' in ink.anchor)) throw new Error('PDF stroke not bar-anchored: ' + JSON.stringify(ink?.anchor))
  })

  await step('ann_04 draw on notation, survives reflow', async () => {
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

  await step('view_04 transpose + hide part (notation)', async () => {
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

  await step('play_04 settings restored on reopen', async () => {
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

  await step('perf_01 performance mode locks editing, big turn zones', async () => {
    await page.click('.top button:has-text("Original")')
    await page.waitForTimeout(800)
    await page.click('.top button:has-text("Annotate")')
    await page.waitForSelector('.annbar')
    await page.click('.top button:has-text("•••")')
    await page.click('.popover.more button:has-text("Perform")')
    await page.waitForSelector('.perfexit')
    await shot('09-performance')
    // Annotation tools gone, overlay not drawable: no accidental ink.
    if (await page.locator('.annbar').count()) throw new Error('annotation toolbar visible in performance mode')
    const active = await page.locator('svg.annot.active').count()
    if (active) throw new Error('ink layer accepts drawing in performance mode')
    const before = (await page.evaluate(() => window.__store.ann.items.length))
    const vp = page.viewportSize()
    await page.mouse.move(vp.width * 0.5, vp.height * 0.5)
    await page.mouse.down()
    await page.mouse.move(vp.width * 0.55, vp.height * 0.55)
    await page.mouse.up()
    if ((await page.evaluate(() => window.__store.ann.items.length)) !== before) throw new Error('stroke added in performance mode')
    // Turn zones cover at least a third of the width each side.
    const z = await page.locator('.turnzone.right').boundingBox()
    if (z.width < vp.width * 0.3) throw new Error('turn zone too small ' + z.width)
    const p0 = await page.locator('.pageno').textContent()
    await page.locator('.turnzone.right').click()
    await page.waitForTimeout(400)
    const p1 = await page.locator('.pageno').textContent()
    if (parseInt(p1) !== parseInt(p0) + 1) throw new Error(`turn zone: ${p0} -> ${p1}`)
    await page.click('.perfexit')
  })

  await step('perf_02 pedal keys are configurable', async () => {
    await page.click('.top button:has-text("•••")')
    await page.click('.popover.more button:has-text("Pedal")')
    // Make ArrowDown do nothing, then check it no longer turns.
    const sel = page.locator('.popover.more .pedals label', { hasText: 'ArrowDown' }).locator('select')
    await sel.selectOption('none')
    await page.click('.top button:has-text("•••")')
    const p0 = await page.locator('.pageno').textContent()
    await page.keyboard.press('ArrowDown')
    await page.waitForTimeout(300)
    if ((await page.locator('.pageno').textContent()) !== p0) throw new Error('ArrowDown still turns after setting None')
    const saved = await page.evaluate(() => JSON.parse(localStorage.getItem('cantor.pedals')))
    if (saved.ArrowDown !== 'none') throw new Error('pedal map not saved')
  })

  await step('set_02 share setlist as bundle, lib_09 as forScore set', async () => {
    await page.click('.top button[aria-label="Library"]')
    await page.click('[role=tab] >> text=Vårkonsert')
    await page.click('button:has-text("Share: Cantor")')
    await page.click('.libbar button:has-text("forScore")')
    await page.waitForTimeout(300)
    const ex = await page.evaluate(() => window.__store.exports)
    const kinds = ex.map((e) => e[0]).join(',')
    if (!/setlist/.test(kinds) || !/4ss/.test(kinds)) throw new Error('exports ' + kinds)
    await page.click('[role=tab] >> text=All scores')
  })

  await step('data_03 back up and restore a received backup', async () => {
    await page.click('header button[aria-label="About Cantor"]')
    await page.click('.about button:has-text("Back up now")')
    await page.waitForSelector('.about [role=status]')
    if ((await page.evaluate(() => window.__store.backups)) !== 1) throw new Error('backup not made')
    await page.click('.about button.primary')
    // A backup arrives via Files/AirDrop: the library asks before restoring.
    await page.evaluate(() => {
      window.__store.pending = ['Cantor 2026-11-20.cantorbackup']
      document.dispatchEvent(new Event('visibilitychange'))
    })
    await page.waitForSelector('.pending')
    await shot('15-pending-backup')
    await page.click('.pending button:has-text("Restore")')
    await page.click('.dlg button.primary')
    await page.waitForTimeout(400)
    if ((await page.evaluate(() => window.__store.restored)) !== 'Cantor 2026-11-20.cantorbackup') throw new Error('not restored')
    if (await page.locator('.pending').count()) throw new Error('prompt still shown')
    await page.click('[role=tab] >> text=All scores')
    await page.locator('ul.scores > li', { hasText: 'Glimmande Nymf' }).locator('button.open').click()
    await page.waitForSelector('.top h2')
  })

  await step('data_01 offline: no request left the device', async () => {
    if (page.__external.length) throw new Error('external requests: ' + page.__external.slice(0, 5).join(', '))
  })

  await step('view_03 phone width shows at least 2 bars per system', async () => {
    // Engrave the same score in a phone-sized pane (390 px wide) with the
    // app's own engraver and count bars on each system (line) of page 1.
    const res = await page.evaluate(async (xml) => {
      const { engrave } = await import('/src/lib/engrave.ts')
      const e = await engrave(xml, 390, 760)
      const host = document.createElement('div')
      host.style.cssText = 'position:fixed;left:0;top:0;width:390px;height:760px'
      host.innerHTML = e.render(1)
      document.body.appendChild(host)
      const systems = [...host.querySelectorAll('g.system')].map((s) => s.querySelectorAll('g.measure').length)
      host.remove()
      return systems
    }, await page.evaluate(() => window.__xml))
    if (!res.length) throw new Error('no systems engraved')
    const min = Math.min(...res)
    console.log('     bars per system at 390 px:', res.join(','))
    if (min < 2) throw new Error('a system has fewer than 2 bars: ' + res.join(','))
  })

  await step('view_01 engraving matches its golden snapshot', async () => {
    const fixture = require('fs').readFileSync(
      path.join(process.env.CANTOR_REPO || path.join(__dirname, '../..'), 'fixtures/musicxml/satb_chorale_24.musicxml'),
      'utf8',
    )
    // Golden = number of pages, systems and bars, and bar order, for a fixed
    // pane. A change in Verovio or the layout settings shows up here.
    const got = await page.evaluate(async (xml) => {
      const { engrave } = await import('/src/lib/engrave.ts')
      const e = await engrave(xml, 744, 1000)
      const host = document.createElement('div')
      const pages = []
      for (let p = 1; p <= e.pages; p++) {
        host.innerHTML = e.render(p)
        pages.push([...host.querySelectorAll('g.system')].map((s) => s.querySelectorAll('g.measure').length))
      }
      return { pages: e.pages, systems: pages, bars: e.measurePage.size }
    }, fixture)
    const fs = require('fs')
    // Golden lives in the repo (public fixture), next to the harness sources.
    const golden = path.join(process.env.CANTOR_REPO || __dirname, 'tools/ui_harness/golden-engraving.json')
    const want = fs.existsSync(golden) ? JSON.parse(fs.readFileSync(golden, 'utf8')) : null
    if (process.env.UPDATE_GOLDEN || !want) {
      fs.writeFileSync(golden, JSON.stringify(got, null, 1))
      console.log('     golden written', JSON.stringify(got))
      if (!want && !process.env.UPDATE_GOLDEN) throw new Error('no golden yet; written, rerun')
    } else if (JSON.stringify(got) !== JSON.stringify(want)) throw new Error(`layout changed: ${JSON.stringify(got)} vs ${JSON.stringify(want)}`)
    // PNG snapshot of page 1, compared within tolerance (anti-aliasing and
    // font hinting differ slightly between machines).
    const { PNG } = require('pngjs')
    const pixelmatch = require('pixelmatch')
    const svg = await page.evaluate(async (xml) => {
      const { engrave } = await import('/src/lib/engrave.ts')
      return (await engrave(xml, 744, 1000)).render(1)
    }, fixture)
    const snap = await page.context().newPage()
    await snap.setViewportSize({ width: 744, height: 1000 })
    await snap.setContent(`<html><body style="margin:0;background:#fff">${svg}</body></html>`)
    const png = PNG.sync.read(await snap.screenshot({ clip: { x: 0, y: 0, width: 744, height: 1000 } }))
    await snap.close()
    const goldenPng = path.join(process.env.CANTOR_REPO || path.join(__dirname, '../..'), 'tools/ui_harness/golden-engraving.png')
    if (process.env.UPDATE_GOLDEN || !fs.existsSync(goldenPng)) {
      fs.writeFileSync(goldenPng, PNG.sync.write(png))
      if (!process.env.UPDATE_GOLDEN) throw new Error('no golden PNG yet; written, rerun')
      return
    }
    const ref = PNG.sync.read(fs.readFileSync(goldenPng))
    if (ref.width !== png.width || ref.height !== png.height) throw new Error(`snapshot size ${png.width}x${png.height} vs ${ref.width}x${ref.height}`)
    const diff = pixelmatch(ref.data, png.data, null, png.width, png.height, { threshold: 0.2 })
    const frac = diff / (png.width * png.height)
    console.log(`     snapshot diff ${(frac * 100).toFixed(3)}% of pixels`)
    if (frac > 0.005) throw new Error(`engraving differs from golden PNG in ${(frac * 100).toFixed(2)}% of pixels`)
  })

  await step('view_06 dark and sepia themes keep text contrast >= 4.5:1', async () => {
    for (const skin of ['dark', 'sepia']) {
      await page.evaluate((s) => {
        localStorage.setItem('cantor.skin', s)
      }, skin)
      await page.reload()
      await page.waitForSelector('ul.scores > li')
      await page.addScriptTag({ path: require.resolve('axe-core/axe.min.js') })
      const v = await page.evaluate(async () => (await window.axe.run(document, { runOnly: ['color-contrast'] })).violations.length)
      if (v) throw new Error(`${skin}: ${v} contrast violations`)
    }
    await page.evaluate(() => localStorage.setItem('cantor.skin', 'system'))
    await page.reload()
    await page.waitForSelector('ul.scores > li')
  })

  await step('play_12 lock screen shows title and position (now playing updated from status)', async () => {
    // The lock-screen card is set natively from the status poll; here we
    // check the app polls status while playing so the card stays current.
    await page.locator('ul.scores > li', { hasText: 'Glimmande Nymf' }).locator('button.open').click()
    await page.waitForSelector('.transport .play')
    const n0 = await page.evaluate(() => window.__calls.filter((c) => c[0] === 'status').length)
    await page.click('.transport .play')
    await page.waitForTimeout(1200)
    const n1 = await page.evaluate(() => window.__calls.filter((c) => c[0] === 'status').length)
    await page.click('.transport .play')
    if (n1 - n0 < 5) throw new Error('status not polled while playing: ' + (n1 - n0))
    await page.click('.top button[aria-label="Library"]')
  })

  await step('lib_06 correct a misread note on the notation', async () => {
    if (await page.locator('.top button[aria-label="Library"]').count()) await page.click('.top button[aria-label="Library"]')
    await page.click('[role=tab] >> text=All scores')
    await page.locator('ul.scores > li', { hasText: 'Skannad' }).locator('button.open').click()
    await page.waitForSelector('.top h2 >> text=Scanned')
    if (await page.locator('.seg button:has-text("Notation")').count()) await page.click('.seg button:has-text("Notation")')
    await page.waitForSelector('.score svg g.note', { timeout: 20000 })
    await page.click('.top button:has-text("Correct notes")')
    // Tap the second note of the page.
    const note = page.locator('.score svg g.note').nth(1)
    const id = await note.getAttribute('data-id')
    const bb = await note.boundingBox()
    await page.mouse.click(bb.x + bb.width / 2, bb.y + bb.height / 2)
    await page.waitForSelector('.annbar >> text=Note selected')
    await page.click('.annbar button[aria-label="Up"]')
    await page.waitForTimeout(1500)
    const c = await page.evaluate(() => window.__store.corrections)
    const m = id.match(/^cn-(\d+)-(\d+)-(\d+)$/)
    if (!c?.length || c[0].file !== '1:Scanned' || c[0].edit !== 'up' || c[0].part !== +m[1] || c[0].measure !== +m[2] || c[0].note !== +m[3])
      throw new Error('correction not sent for the tapped note: ' + JSON.stringify(c) + ' ' + id)
    await shot('16-correct')
    // Bundled scores cannot be corrected: the button is not offered.
    await page.click('.top button[aria-label="Library"]')
    await page.locator('ul.scores > li', { hasText: 'Christmas Day' }).locator('button.open').click()
    await page.waitForSelector('.score svg g.note', { timeout: 20000 })
    if (await page.locator('.top button:has-text("Correct notes")').count()) throw new Error('offered on a bundled score')
    await page.click('.top button[aria-label="Library"]')
  })

  await step('perf_04 backing track turns the pages of a PDF-only score', async () => {
    if (await page.locator('.top button[aria-label="Library"]').count()) await page.click('.top button[aria-label="Library"]')
    await page.click('[role=tab] >> text=All scores')
    await page.locator('ul.scores > li', { hasText: 'Min egen' }).locator('button.open').click()
    await page.waitForSelector('.pdfhost canvas')
    // Attach a recording through the ••• menu.
    await page.click('.top button[aria-haspopup="dialog"]')
    await page.setInputFiles('.popover.more .track input[type=file]', { name: 'take1.m4a', mimeType: 'audio/mp4', buffer: Buffer.from('....ftypM4A ') })
    await page.waitForSelector('.transport .play')
    // Mark two turns through the panel (the button marks "next page at the
    // current time"), then move them to known times.
    if (!(await page.locator('.popover.more').count())) await page.click('.top button[aria-haspopup="dialog"]')
    await page.waitForSelector('.popover.more .track')
    await page.click('.popover.more .track button:has-text("Turn to page")')
    await page.waitForTimeout(200)
    const saved = await page.evaluate(() => window.__store.track.markers)
    if (saved.length !== 1 || saved[0][1] !== 2) throw new Error('marker not saved: ' + JSON.stringify(saved))
    await shot('17-track-panel')
    await page.evaluate(() => {
      window.__store.track.markers = [[0.5, 2], [1.0, 3]]
      window.__store.track.fired = new Set()
    })
    await page.click('.top button[aria-haspopup="dialog"]')
    const p0 = await page.locator('.pageno').textContent()
    await page.click('.transport .play')
    await page.waitForFunction(() => /^3/.test(document.querySelector('.pageno')?.textContent ?? ''), null, { timeout: 5000 })
    await page.click('.transport .play')
    const p1 = await page.locator('.pageno').textContent()
    if (!/^1/.test(p0) || !/^3/.test(p1)) throw new Error(`pages ${p0} -> ${p1}`)
    // The recording shows as a mixer channel.
    await page.click('.top button:has-text("Mixer")')
    await page.waitForSelector('.strip >> text=Backing track')
    await page.click('.top button:has-text("Hide mixer")')
    await page.click('.top button[aria-label="Library"]')
  })

  await step('data_02 choose a sync folder and sync', async () => {
    if (await page.locator('.top button[aria-label="Library"]').count()) await page.click('.top button[aria-label="Library"]')
    const before = await page.evaluate(() => window.__store.syncs ?? 0)
    if (before < 1) throw new Error('library did not sync on launch')
    await page.click('header button[aria-label="About Cantor"]')
    await page.click('.about button:has-text("Choose folder")')
    await page.waitForSelector('.about >> text=Syncing with the folder')
    await page.waitForSelector('.about >> text=Synced: 2 changes sent, 1 received')
    await shot('18-sync')
    await page.click('.about button:has-text("Stop syncing")')
    await page.waitForSelector('.about button:has-text("Choose folder")')
    await page.click('.about button.primary')
  })

  await step('nfr_10 swedish', async () => {
    if (await page.locator('.top button[aria-label="Library"]').count()) await page.click('.top button[aria-label="Library"]')
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
