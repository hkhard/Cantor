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
  // The vite dev server's own client sometimes fails to load during the many
  // reloads and viewport changes ("Cannot load …/@vite/client", then
  // "Importing a module script failed"). That is the test server, not the app;
  // a real app error still fails the run.
  let viteGlitch = false
  page.on('pageerror', (e) => {
    if (viteGlitch && /Importing a module script failed/.test(e.message)) return
    errors.push('pageerror: ' + e.message)
  })
  page.on('console', (m) => {
    if (m.type() !== 'error') return
    if (/\/@vite\/client|\/node_modules\/vite\/dist\/client\//.test(m.text())) {
      viteGlitch = true
      return
    }
    errors.push('console: ' + m.text())
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

  await step('lib_00 cold start shows the flapping Songbird, then the library', async () => {
    await page.waitForSelector('.splash img[src="/songbird-flap.gif"]', { timeout: 3000 })
    await shot('00-splash')
    await page.waitForSelector('.splash', { state: 'detached', timeout: 5000 })
  })
  await page.waitForSelector('text=Lacrymosa')
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
    await page.locator('ul.scores > li', { hasText: 'Lacrymosa' }).locator('button.info').click()
    await page.waitForSelector('.metasheet')
    await shot('02-meta')
    const inputs = page.locator('.metasheet input')
    await inputs.nth(1).fill('Bellman')
    await inputs.nth(6).fill('vår, konsert')
    await page.locator('.metasheet .stars').first().locator('button').nth(3).click()
    await page.locator('.metasheet button.primary').click()
    await page.waitForSelector('.metasheet', { state: 'detached' })
    const m = await page.evaluate(() => window.__store.metadata['1:Lacrymosa'])
    if (m.composer !== 'Bellman' || m.tags.length !== 2 || m.rating !== 4) throw new Error(JSON.stringify(m))
    await page.waitForSelector('text=Bellman')
  })

  await step('lib_03 read title and credits from the PDF page (on-device text recognition)', async () => {
    await page.locator('ul.scores > li', { hasText: 'Min egen' }).locator('button.info').click()
    await page.waitForSelector('.metasheet')
    await page.click('.metasheet button >> text=Read from the page')
    await page.waitForSelector('.metasheet [role=status]')
    const bytes = await page.evaluate(() => window.__store.ocrBytes)
    // pdf.js rendered page 1 to a real PNG and sent it.
    if (!(bytes > 10000)) throw new Error('no page image sent ' + bytes)
    // Keep the exact bytes the UI sent, so the real recogniser can be run on
    // them (tools/ui_harness/run.sh ocr-check; cargo example read_title).
    const png = await page.evaluate(() => window.__store.ocrPng)
    fs.writeFileSync(path.join(out, 'ocr-input.png'), Buffer.from(png))
    const vals = await page.locator('.metasheet input').evaluateAll((els) => els.map((e) => e.value))
    if (vals[0] !== 'Vårvindar friska' || vals[1] !== 'Svensk folkmelodi' || vals[2] !== 'Hugo Alfvén') throw new Error('fields ' + vals.slice(0, 3))
    const tags = await page.locator('.metasheet input').nth(6).inputValue()
    if (!tags.includes('Words: Julia Nyberg') || !tags.includes('SATB')) throw new Error('tags ' + tags)
    await shot('31-read-title-page')
    await page.click('.metasheet button >> text=Cancel')
  })

  await step('lib_04 start view: group by tags, pick one, restored after restart', async () => {
    // Default: setlists grouping with "All scores".
    if ((await page.locator('.viewsel select').inputValue()) !== 'setlists') throw new Error('default grouping')
    await page.selectOption('.viewsel select', 'tags')
    await page.click('[role=tab] >> text=vår')
    const titles = await page.locator('ul.scores > li .t > span:first-child').allInnerTexts()
    if (titles.length !== 1 || titles[0] !== 'Lacrymosa') throw new Error('filter ' + titles)
    // Scores without tags are reachable under "None".
    await page.click('[role=tab] >> text=None')
    const rest = await page.locator('ul.scores > li').count()
    if (rest !== 3) throw new Error('none ' + rest)
    await page.click('[role=tab] >> text=vår')
    await shot('32-group-tags')
    await page.waitForTimeout(500)
    const saved = await page.evaluate(() => window.__store.home)
    if (saved.group !== 'tags' || saved.tab !== 'vår') throw new Error('not saved ' + JSON.stringify(saved))
    // Restart: the app opens on the same view.
    await page.reload()
    await page.waitForSelector('ul.scores > li')
    if ((await page.locator('.viewsel select').inputValue()) !== 'tags') throw new Error('grouping not restored')
    const sel = (await page.locator('[role=tab][aria-selected=true]').innerText()).replace(/\s+/g, ' ').trim()
    if (sel !== 'vår 1') throw new Error('tab not restored: ' + sel)
    // Back to the default for the following steps.
    await page.selectOption('.viewsel select', 'setlists')
    await page.waitForTimeout(400)
  })

  await step('lib_05 suggest combining a MusicXML and a PDF of the same piece', async () => {
    // A MusicXML ("Ave verum") and its PDF under a download name arrived
    // separately; the backend says they match (title on the PDF's page 1).
    await page.evaluate(() => {
      window.__addScores([
        { file: '1:aveverum', title: 'Ave verum', playable: true, pdf: false, user: true },
        { file: '1:score-b503fb', title: 'score-b503fb', playable: false, pdf: true, user: true },
      ])
      window.__store.pairs = [['1:aveverum', '1:score-b503fb']]
      document.dispatchEvent(new Event('visibilitychange'))
    })
    await page.click('[role=tab] >> text=All scores')
    // Coming back to the app reloads the library and asks.
    await page.waitForSelector('.pending >> text=look like the same piece')
    await shot('33-combine-ask')
    const before = await page.locator('ul.scores > li').count()
    await page.click('.pending button:has-text("Combine")')
    await page.waitForSelector('.pending >> text=look like the same piece', { state: 'detached' })
    const combined = await page.evaluate(() => window.__store.combined)
    if (!combined || combined[0] !== '1:aveverum') throw new Error('not combined ' + combined)
    const after = await page.locator('ul.scores > li').count()
    if (after !== before - 1) throw new Error(`cards ${before} -> ${after}`)
    const card = page.locator('ul.scores > li', { hasText: 'Ave verum' })
    if (!(await card.locator('text=PDF').count())) throw new Error('combined card has no PDF badge')
    // Leave the library as the later steps expect it.
    await page.evaluate(() => {
      window.__removeScore('1:aveverum')
      document.dispatchEvent(new Event('visibilitychange'))
    })
    await page.waitForSelector('ul.scores > li >> text=Ave verum', { state: 'detached' })
  })

  await step('lib_05 combine later from Info after answering no', async () => {
    // The user said "No, they differ" (or the names never matched): the
    // pair is no longer suggested, but Info still offers "Combine with".
    await page.evaluate(() => {
      window.__addScores([
        { file: '1:gloria', title: 'Gloria', playable: true, pdf: false, user: true },
        { file: '1:scan-77', title: 'scan-77', playable: false, pdf: true, user: true },
      ])
      window.__store.pairs = [['1:gloria', '1:scan-77']]
      document.dispatchEvent(new Event('visibilitychange'))
    })
    await page.waitForSelector('.pending >> text=look like the same piece')
    await page.click('.pending button:has-text("No, they differ")')
    await page.waitForSelector('.pending >> text=look like the same piece', { state: 'detached' })
    // From the PDF's side: Info, pick the notes, combine.
    await page.click('ul.scores > li:has-text("scan-77") button.info')
    const sheet = page.locator('.metasheet')
    await sheet.locator('select[aria-label="Combine with"]').selectOption('1:gloria')
    await sheet.locator('button:has-text("Combine")').click()
    await page.waitForSelector('.metasheet', { state: 'detached' })
    const combined = await page.evaluate(() => window.__store.combined)
    if (!combined || combined[0] !== '1:gloria' || combined[1] !== '1:scan-77') throw new Error('not combined ' + combined)
    const card = page.locator('ul.scores > li', { hasText: 'Gloria' })
    if (!(await card.locator('text=PDF').count())) throw new Error('combined card has no PDF badge')
    if (await page.locator('ul.scores > li', { hasText: 'scan-77' }).count()) throw new Error('PDF card still there')
    // Bundled and complete scores offer nothing to combine.
    await page.click('ul.scores > li:has-text("Gloria") button.info')
    if (await page.locator('.metasheet select[aria-label="Combine with"]').count()) throw new Error('complete score offers combine')
    await page.click('.metasheet button:has-text("Cancel")')
    await page.evaluate(() => {
      window.__removeScore('1:gloria')
      document.dispatchEvent(new Event('visibilitychange'))
    })
    await page.waitForSelector('ul.scores > li >> text=Gloria', { state: 'detached' })
  })

  await step('vox_00 nothing in the UI mentions sung lyrics before singing ships', async () => {
    // Singing is parked (plan section 8): no UI text may promise it.
    const txt = (await page.locator('body').innerText()).toLowerCase()
    for (const w of ['sung lyrics', 'sjungen text', 'singing voice', 'sångröst']) {
      if (txt.includes(w)) throw new Error(`UI mentions "${w}"`)
    }
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
    for (const t of ['Christmas Day', 'Lacrymosa']) {
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
    if (first !== '1:Lacrymosa') throw new Error('reorder failed ' + first)
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
    const n = await cards.count()
    const last = await cards.nth(n - 1).boundingBox()
    const frames = await longDrag(cards.nth(0), { x: last.x + 6, y: last.y + last.height / 2 }, { steps: 50, dwell: 300 })
    const domChangesDuringDrag = await page.evaluate(() => {
      const n = window.__moves
      window.__mo.disconnect()
      return n
    })
    checkFrames(frames, 'library')
    await page.waitForTimeout(300)
    const t1 = await titles()
    const want = [...t0.slice(1), t0[0]]
    if (JSON.stringify(t1) !== JSON.stringify(want)) throw new Error(`order ${t1} want ${want}`)
    const saved = await page.evaluate(() => window.__store.scoreOrder.length)
    if (saved !== n) throw new Error('order not saved')
    // Only the drop changes the DOM (one reorder), never mid-drag.
    if (domChangesDuringDrag > 12) throw new Error('DOM churn during drag ' + domChangesDuringDrag)
    // The saved order is what the backend gets back on the next start
    // (persistence itself: set_01_manual_order_… in the library crate).
    const t2 = await page.evaluate(async () => (await window.__TAURI_INTERNALS__.invoke('list_scores', {})).filter((s) => s.user).map((s) => s.title))
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
    await page.waitForSelector('.top h2 >> text=Lacrymosa')
    await page.waitForSelector('.pdfhost canvas')
    await page.waitForTimeout(1500)
    await shot('04-reader-pdf')
    await page.click('.top button[aria-label="Next piece"]')
    await page.waitForSelector('.top h2 >> text=Christmas')
    await page.click('.top button[aria-label="Previous piece"]')
    await page.waitForSelector('.top h2 >> text=Lacrymosa')
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

  await step('play_09 choose an instrument per voice and a voice set for the choir', async () => {
    if (!(await page.locator('aside.mixer').count())) await page.locator('.top button:has(svg)').last().click()
    await page.waitForSelector('aside.mixer')
    await page.click('aside.mixer button.vstoggle')
    await page.waitForSelector('.instsel select')
    await shot('34-voices')
    // One voice: Bas 1 -> Church Organ (GM 19); saved for this score.
    await page.locator('.instsel', { hasText: 'Bas 1' }).locator('select').selectOption('19')
    const calls = await page.evaluate(() => window.__store.instrumentCalls)
    const last = calls?.at(-1)
    if (!last || last[1] !== 2 || last[2] !== 19) throw new Error('set_instrument ' + JSON.stringify(calls))
    if (!(await page.locator('.strip', { hasText: 'Bas 1' }).innerText()).includes('Church Organ')) throw new Error('strip not updated')
    // A whole voice set: Choir aahs for every voice; the score reopens with it.
    await page.locator('.vsbar select').selectOption('choir-aahs')
    await page.waitForFunction(() => [...document.querySelectorAll('.strip .inst')].every((e) => e.textContent === 'Choir Aahs'))
    // Make it the default for male choirs, and save the current mix as an own set.
    await page.click('.vsbar button:has-text("Make default")')
    if ((await page.evaluate(() => window.__store.defaultMale)) !== 'choir-aahs') throw new Error('default not saved')
    await page.click('.vsbar button:has-text("Save as set")')
    await page.waitForSelector('.dlg input')
    await page.fill('.dlg input', 'Vår TTBB')
    await page.click('.dlg button.primary')
    await page.waitForFunction(() => (window.__store.ownSets ?? []).some((v) => v.name_en === 'Vår TTBB' && v.roles.tenor?.length === 2))
    // Back to Classic for the steps that follow.
    await page.locator('.vsbar select').selectOption('classic')
    await page.waitForTimeout(300)
  })

  await step('play_04 custom preset saved per score', async () => {
    await page.locator('.strip').nth(0).locator('button.m').click()
    await page.click('text=Save mix')
    await page.fill('.dlg input', 'Utan T1')
    await page.click('.dlg button.primary')
    await page.waitForTimeout(600)
    const s = await page.evaluate(() => window.__store.settings['1:Lacrymosa'])
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
    // Start inside bar 1 of the fixture's measure map, so the stroke lands
    // on a bar whatever the score's layout (title space, indents).
    const m0 = await page.evaluate(() => window.__boxes?.[0] ?? null)
    const sx = m0 ? b.x + b.width * (m0.x + m0.w * 0.2) : b.x + 100
    const sy = m0 ? b.y + b.height * (m0.y + m0.h * 0.4) : b.y + 200
    await page.mouse.move(sx, sy)
    await page.mouse.down()
    for (let i = 1; i <= 10; i++) await page.mouse.move(sx + i * 15, sy + i * 3)
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
      // The smallest bar box holding the centre: a bar's box can include
      // text that runs far past it (Lacrymosa's lyric line in bar 1).
      let inBar = -1
      res.bars.forEach(([l, r, t, b], i) => {
        if (cx >= l && cx <= r && cy >= t && cy <= b && (inBar < 0 || (r - l) * (b - t) < (res.bars[inBar][1] - res.bars[inBar][0]) * (res.bars[inBar][3] - res.bars[inBar][2]))) inBar = i
      })
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
    // Staves per system: fewer parts fit more systems on a page.
    const before = await page.evaluate(() => document.querySelector('.score svg g.system')?.querySelectorAll('g.staff').length ?? 0)
    await page.locator('.popover.more .check input').first().uncheck()
    await page.waitForTimeout(2500)
    await page.click('.top button:has-text("•••")')
    const after = await page.evaluate(() => document.querySelector('.score svg g.system')?.querySelectorAll('g.staff').length ?? 0)
    await shot('08-notation-hidden')
    if (!(after < before)) throw new Error(`staves ${before} -> ${after}`)
    const s = await page.evaluate(() => window.__store.settings['1:Lacrymosa'])
    if (s.transpose !== 1 || s.hidden_parts[0] !== 0) throw new Error(JSON.stringify(s))
  })

  await step('play_04 settings restored on reopen', async () => {
    await page.click('.top button[aria-label="Library"]')
    await page.waitForSelector('text=New setlist')
    await page.click('[role=tab] >> text=All scores')
    await page.locator('ul.scores > li', { hasText: 'Lacrymosa' }).locator('button.open').click()
    await page.waitForSelector('.top h2')
    await page.waitForTimeout(1500)
    const last = await page.evaluate(() => window.__calls.filter((c) => c[0] === 'open_score').pop()[1])
    if (last.transpose !== 1) throw new Error('transpose not restored')
    const view = await page.locator('.top .seg button.on').first().textContent()
    if (!/Notation/.test(view)) throw new Error('view not restored: ' + view)
  })

  await step('perf_01 performance mode locks editing, big turn zones', async () => {
    await page.click('.top button:has-text("Original")')
    await page.waitForTimeout(800)
    await page.click('.top button:has-text("Annotate")')
    await page.waitForSelector('.annbar')
    await page.click('.modeswitch button:has-text("Performance")')
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

  await step('perf_07 practice and performance modes: small controls, step and jump through pages and pieces', async () => {
    // A setlist of two pieces, opened from the library (added beside the
    // others, removed again at the end).
    await page.evaluate(() => {
      window.__store.setlists = [...window.__store.setlists, { id: 'gig', name: 'Gig', items: [
        { kind: 'score', score: '1:Lacrymosa', start_page: null },
        { kind: 'score', score: '1:Christmas', start_page: null },
      ] }]
    })
    const wasOpen = await page.locator('.top h2').innerText().catch(() => '')
    if (await page.locator('.top button[aria-label="Library"]').count()) await page.click('.top button[aria-label="Library"]')
    await page.reload()
    await page.waitForSelector('ul.scores > li')
    await page.click('[role=tab] >> text=Gig')
    await page.locator('ol.setlist > li').first().locator('button.open').click()
    await page.waitForSelector('.pdfhost canvas')
    // Practice: rehearsal controls are there.
    const mode = page.locator('.modeswitch')
    if (!(await mode.locator('button[aria-pressed="true"]:has-text("Practice")').count())) throw new Error('not in practice mode')
    if (!(await page.locator('.transport').isVisible())) throw new Error('no transport in practice')
    await mode.locator('button:has-text("Performance")').click()
    await page.waitForSelector('.perfbar')
    await shot('53-performance-bar')
    // Performance: no rehearsal controls, a small bar.
    for (const sel of ['.transport', '.top', '.mixer']) if (await page.locator(sel).isVisible().catch(() => false)) throw new Error(sel + ' visible in performance')
    const bar = await page.locator('.perfbar').boundingBox()
    const vp = page.viewportSize()
    if (bar.height > 72 || bar.width > vp.width * 0.9) throw new Error('performance bar is not small: ' + JSON.stringify(bar))
    const pg = async () => (await page.locator('.perfbar select[aria-label="Go to page"] option:checked').innerText()).trim()
    const pgsel = async () => (await page.locator('.perfbar select[aria-label="Go to page"] option:checked').innerText()).trim()
    if (!/^1\s*\/\s*5$/.test(await pgsel())) throw new Error('page indicator ' + (await pgsel()))
    await page.click('.perfbar button[aria-label="Next page"]')
    await page.waitForTimeout(300)
    if (!/^2\s*\//.test(await pg())) throw new Error('next page: ' + (await pg()))
    // Jump to a chosen page.
    await page.selectOption('.perfbar select[aria-label="Go to page"]', '4')
    await page.waitForTimeout(300)
    if (!/^4\s*\//.test(await pg())) throw new Error('jump to page: ' + (await pg()))
    await page.click('.perfbar button[aria-label="Previous page"]')
    await page.waitForTimeout(300)
    if (!/^3\s*\//.test(await pg())) throw new Error('previous page: ' + (await pg()))
    // Pieces in the setlist: next piece, and jump back to the first.
    const piece = () => page.evaluate(() => document.querySelector('.perfbar select[aria-label="Go to piece"]')?.selectedOptions[0]?.textContent ?? '')
    if (!(await piece()).startsWith('1/2')) throw new Error('piece position missing: ' + (await piece()))
    await page.click('.perfbar button[aria-label="Next piece"]')
    await page.waitForFunction(() => document.querySelector('.perfbar select[aria-label="Go to piece"]')?.selectedOptions[0]?.textContent.startsWith('2/2'))
    if (!(await page.locator('.perfbar').isVisible())) throw new Error('left performance mode on next piece')
    await page.selectOption('.perfbar select[aria-label="Go to piece"]', '0')
    await page.waitForFunction(() => document.querySelector('.perfbar select[aria-label="Go to piece"]')?.selectedOptions[0]?.textContent.startsWith('1/2'))
    // Back to practice: rehearsal controls return.
    await page.click('.perfbar button:has-text("Practice")')
    await page.waitForSelector('.transport')
    if (await page.locator('.perfbar').count()) throw new Error('performance bar left in practice')
    // Leave things as the later steps expect: no Gig list, the same score open.
    await page.evaluate(() => {
      window.__store.setlists = window.__store.setlists.filter((s) => s.id !== 'gig')
      document.activeElement?.blur()
    })
    await page.click('.top button[aria-label="Library"]')
    await page.click('[role=tab] >> text=All scores').catch(() => {})
    const title = wasOpen.split('·')[0].trim() || 'Lacrymosa'
    await page.locator('ul.scores > li', { hasText: title }).first().locator('button.open').click()
    await page.waitForSelector('.top h2')
    await page.waitForTimeout(800)
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
    await page.click('button:has-text("Share: ChoirBird")')
    await page.click('.libbar button:has-text("forScore")')
    await page.waitForTimeout(300)
    const ex = await page.evaluate(() => window.__store.exports)
    const kinds = ex.map((e) => e[0]).join(',')
    if (!/setlist/.test(kinds) || !/4ss/.test(kinds)) throw new Error('exports ' + kinds)
    await page.click('[role=tab] >> text=All scores')
  })

  await step('set_03 instructions per score: write, bar reference jumps, member gets their part', async () => {
    await page.locator('ul.scores > li', { hasText: 'Lacrymosa' }).locator('button.open').click()
    await page.waitForSelector('.pdfhost canvas')
    if (await page.locator('.inspanel').count()) throw new Error('panel open with no instructions')
    await page.click('.top button[aria-label="Instructions"]')
    await page.click('.inspanel button:has-text("Edit")')
    await page.fill('.inspanel textarea', 'Breathe before m. 3.\n\n**Piano** from bar 2, see [notes](https://example.org).')
    await page.fill('.inspanel input[aria-label="Name or group"]', 'Bo')
    await page.locator('.inspanel select[aria-label="Part"]').selectOption('Tenor 1')
    await page.click('.inspanel button:has-text("Save")')
    await page.waitForSelector('.inspanel .barref')
    const saved = await page.evaluate(() => window.__store.instructions['1:Lacrymosa'])
    if (!saved.text.includes('m. 3') || saved.parts[0].who !== 'Bo' || saved.parts[0].part !== 'Tenor 1') throw new Error('saved ' + JSON.stringify(saved))
    if ((await page.locator('.inspanel .ins p').count()) !== 2) throw new Error('paragraphs')
    if ((await page.locator('.inspanel .ins strong', { hasText: 'Piano' }).count()) !== 1) throw new Error('bold')
    const href = await page.locator('.inspanel .ins a').getAttribute('href')
    if (href !== 'https://example.org') throw new Error('link ' + href)
    if ((await page.locator('.inspanel .barref').count()) !== 2) throw new Error('bar refs')
    // "m. 3" seeks to bar 3 (mock: 2 s per bar, so 4 s).
    await page.click('.inspanel .barref >> text=m. 3')
    await page.waitForTimeout(200)
    const seek = await page.evaluate(() => window.__calls.filter((c) => c[0] === 'seek').at(-1))
    if (!seek || seek[1].seconds !== 4) throw new Error('seek ' + JSON.stringify(seek))
    // Pick "I am Bo": the part is named and marked in the mixer.
    await page.locator('.inspanel label:has-text("I am") select').selectOption('Bo')
    await page.waitForSelector('.inspanel >> text=Your part: Tenor 1')
    if ((await page.evaluate(() => window.__store.member)) !== 'Bo') throw new Error('member not saved')
    if (!(await page.locator('aside.mixer').count())) await page.click('.top button[aria-label="Mixer"]')
    await page.waitForSelector('.strip.hinted')
    if ((await page.locator('.strip.hinted .label').textContent()).trim() !== 'Tenor 1') throw new Error('wrong part marked')
    await shot('40-instructions')
    await page.click('.top button[aria-label="Library"]')
  })

  await step('set_03 setlist instructions on the setlist page', async () => {
    await page.click('[role=tab] >> text=Vårkonsert')
    await page.click('.setins button:has-text("Edit")')
    await page.fill('.setins textarea', 'Meet at **17.30**. Black clothes.')
    await page.click('.setins button:has-text("Save")')
    await page.waitForSelector('.setins strong >> text=17.30')
    const sl = await page.evaluate(() => window.__store.setlists.find((s) => s.name === 'Vårkonsert'))
    if (sl.instructions !== 'Meet at **17.30**. Black clothes.') throw new Error('set instructions ' + sl.instructions)
  })

  await step('set_05 note per setlist item, shown in the list and the reader', async () => {
    const first = page.locator('ol.setlist > li').first()
    await first.locator('button.notebtn').click()
    await page.waitForSelector('.dlg input')
    await page.fill('.dlg input', 'Start at bar 9')
    await page.click('.dlg button.primary')
    await page.waitForSelector('ol.setlist .itemnote >> text=Start at bar 9')
    const it = await page.evaluate(() => window.__store.setlists.find((s) => s.name === 'Vårkonsert').items[0])
    if (it.note !== 'Start at bar 9') throw new Error('note ' + JSON.stringify(it))
    await shot('41-setlist-instructions')
    // Opening the set shows set instructions, the item note and the piece's own.
    await page.click('text=Play setlist')
    await page.waitForSelector('.inspanel')
    const txt = await page.locator('.inspanel').innerText()
    for (const want of ['Meet at', 'Start at bar 9', 'Breathe before']) if (!txt.includes(want)) throw new Error('panel lacks ' + want)
    if (!(await page.locator('.inspanel .barref >> text=bar 9').count())) throw new Error('note bar ref')
    await page.click('.top button[aria-label="Library"]')
  })

  await step('set_04 a received set opens with its instructions and offers practise my part', async () => {
    await page.evaluate(() => {
      window.__store.setlists.push({ id: 'rx', name: 'Mottagen', instructions: 'Welcome! Learn **m. 5** first.', items: [{ kind: 'score', score: '1:Lacrymosa', start_page: null, note: '' }] })
      window.__store.receivedSet = 'rx'
      window.__store.practice = { '1:Lacrymosa': ['Tenor 1', 'Bas 1'] }
      document.dispatchEvent(new Event('visibilitychange'))
    })
    await page.waitForSelector('[role=status] >> text=You received the setlist')
    if ((await page.locator('h2.setname').textContent()).trim() !== 'Mottagen') throw new Error('not on the received set')
    await page.waitForSelector('.setins strong >> text=m. 5')
    await shot('42-received-set')
    await page.click('text=Play setlist')
    await page.waitForSelector('.inspanel')
    await page.waitForSelector('.inspanel button:has-text("Practise my part (Tenor 1)")')
    if ((await page.locator('.inspanel .tracks button').count()) !== 2) throw new Error('practice tracks')
    await page.click('.top button[aria-label="Library"]')
    await page.click('[role=tab] >> text=All scores')
  })

  await step('about licence: About states AGPL-3.0 and where the source is', async () => {
    await page.click('header button[aria-label="About ChoirBird"]')
    const txt = await page.locator('.about').innerText()
    if (!txt.includes('GNU Affero General Public License, version 3')) throw new Error('no AGPL notice')
    if (!txt.includes('app store')) throw new Error('no app store permission mentioned')
    if (!/Source code for this version:\s*https:\/\/github\.com\/\S+\/tree\/\S+/.test(txt)) throw new Error('no source link to the exact version')
    if (!/Version:\s*\S+/.test(txt)) throw new Error('no version shown')
    if (!txt.includes('THIRD_PARTY_NOTICES.md')) throw new Error('no third-party notices')
    await page.click('.about button.primary')
  })

  await step('data_03 back up and restore a received backup', async () => {
    await page.click('header button[aria-label="About ChoirBird"]')
    await page.click('.about button:has-text("Back up now")')
    await page.waitForSelector('.about [role=status]')
    if ((await page.evaluate(() => window.__store.backups)) !== 1) throw new Error('backup not made')
    await page.click('.about button.primary')
    // A backup arrives via Files/AirDrop: the library asks before restoring.
    await page.evaluate(() => {
      window.__store.pending = ['ChoirBird 2026-11-20.cantorbackup']
      document.dispatchEvent(new Event('visibilitychange'))
    })
    await page.waitForSelector('.pending')
    await shot('15-pending-backup')
    await page.click('.pending button:has-text("Restore")')
    await page.click('.dlg button.primary')
    await page.waitForTimeout(400)
    if ((await page.evaluate(() => window.__store.restored)) !== 'ChoirBird 2026-11-20.cantorbackup') throw new Error('not restored')
    if (await page.locator('.pending').count()) throw new Error('prompt still shown')
    await page.click('[role=tab] >> text=All scores')
    await page.locator('ul.scores > li', { hasText: 'Lacrymosa' }).locator('button.open').click()
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
    }, require('fs').readFileSync(
      // A hymn-like 4/4 SATB score: Lacrymosa's 12/8 piano bars are too dense
      // for two to a phone line, and that is correct engraving.
      path.join(process.env.CANTOR_REPO || path.join(__dirname, '../..'), 'fixtures/musicxml/satb_chorale_24.musicxml'),
      'utf8',
    ))
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
    await page.locator('ul.scores > li', { hasText: 'Lacrymosa' }).locator('button.open').click()
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
    await page.click('header button[aria-label="About ChoirBird"]')
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
    await page.click('header button[aria-label="About ChoirBird"]')
    await page.selectOption('.about select', 'sv')
    await page.waitForSelector('text=Licenser')
    await shot('10-about-sv')
    await page.click('.about button.primary')
    await page.waitForSelector('text=Alla noter')
    await shot('11-library-sv')
  })

  // Phase P first run. Swedish is on after the nfr_10 step above.
  await step('lib_01 Getting started: how to add scores and the demo; the demo is never in the score list', async () => {
    if (await page.locator('.top button[aria-label="Bibliotek"]').count()) await page.click('.top button[aria-label="Bibliotek"]')
    // Dismissed (the default in these tests): no card, no demo anywhere.
    if (await page.locator('.welcome').count()) throw new Error('Getting started shown after "do not show again"')
    if (await page.locator('ul.scores > li', { hasText: '(demo)' }).count()) throw new Error('demo listed among scores')
    await page.evaluate(() => {
      window.__store.hints = []
      for (const f of ['1:Mine', '1:Scanned']) window.__removeScore(f, true)
    })
    await page.reload()
    await page.waitForSelector('.welcome .trycard')
    const txt = await page.locator('.welcome').innerText()
    for (const w of ['Importera', 'AirDrop', 'Filer', 'Lacrymosa']) if (!txt.includes(w)) throw new Error('welcome lacks ' + w)
    if (await page.locator('ul.scores > li', { hasText: '(demo)' }).count()) throw new Error('demo listed among scores')
    await shot('40-first-run-sv')
  })

  await step('lib_01 intro hint comes back until "do not show again" is ticked, then stays away', async () => {
    await page.evaluate(() => { window.__store.hints = [] })
    await page.reload()
    await page.waitForSelector('.welcome .trycard')
    await page.click('.welcome .trycard')
    await page.waitForSelector('.readerhint')
    const txt = await page.locator('.readerhint').innerText()
    if (!/takt/.test(txt) || !/mixer/i.test(txt)) throw new Error('hint text: ' + txt)
    await shot('41-reader-hint-sv')
    // OK without the box: the hint is shown again next time.
    await page.click('.readerhint button.primary')
    await page.waitForSelector('.readerhint', { state: 'detached' })
    if ((await page.evaluate(() => window.__store.hints)).includes('reader')) throw new Error('saved without the box ticked')
    await page.reload()
    await page.waitForSelector('.welcome .trycard')
    await page.click('.welcome .trycard')
    await page.waitForSelector('.readerhint')
    // Tick "do not show again": remembered in the store, never shown again.
    await page.check('.readerhint label:has-text("Visa inte igen") input')
    await page.click('.readerhint button.primary')
    await page.waitForSelector('.readerhint', { state: 'detached' })
    if (!(await page.evaluate(() => window.__store.hints)).includes('reader')) throw new Error('hint not saved in the store')
    await page.reload()
    await page.waitForSelector('ul.scores > li')
    if (await page.locator('.welcome').count()) throw new Error('Getting started still shown')
    await page.locator('ul.scores > li', { hasText: 'Lacrymosa' }).first().locator('button.open').click()
    await page.waitForSelector('.pdfhost canvas')
    await page.waitForTimeout(500)
    if (await page.locator('.readerhint').count()) throw new Error('hint shown again')
  })

  await step('lib_01 "do not show again" hides only Getting started; About brings it back', async () => {
    await page.evaluate(() => { window.__store.hints = [] })
    await page.reload()
    await page.waitForSelector('.welcome')
    const own = await page.locator('ul.scores > li').count()
    await page.check('.welcome label:has-text("Visa inte igen") input')
    await page.click('.welcome button:has-text("Stäng")')
    await page.waitForSelector('.welcome', { state: 'detached' })
    if ((await page.locator('ul.scores > li').count()) !== own) throw new Error('scores disappeared with Getting started')
    if (!(await page.locator('ul.scores > li', { hasText: 'Lacrymosa' }).count())) throw new Error('own Lacrymosa gone')
    await page.reload()
    await page.waitForSelector('ul.scores > li')
    if (await page.locator('.welcome').count()) throw new Error('Getting started back after reload')
    // About > Show Getting started.
    await page.click('header button[aria-label="Om ChoirBird"]')
    await page.click('.about button:has-text("Visa Kom igång")')
    await page.click('.about button.primary')
    await page.waitForSelector('.welcome .trycard')
    if ((await page.evaluate(() => window.__store.hints)).includes('reader')) throw new Error('hint not cleared')
  })

  await step('lib_01 hint has no animation with reduced motion', async () => {
    await page.emulateMedia({ reducedMotion: 'reduce' })
    await page.evaluate(() => { window.__store.hints = [] })
    await page.reload()
    await page.waitForSelector('.welcome .trycard')
    await page.click('.welcome .trycard')
    await page.waitForSelector('.readerhint')
    const anim = await page.locator('.readerhint').evaluate((e) => getComputedStyle(e).animationName)
    if (anim !== 'none') throw new Error('animation: ' + anim)
    await page.click('.readerhint button.primary')
    await page.emulateMedia({ reducedMotion: 'no-preference' })
  })

  await step('view_07 landscape: the PDF page fills the width, the system is whole', async () => {
    await page.setViewportSize({ width: 1133, height: 744 })
    await page.evaluate(() => localStorage.setItem('cantor.lang', 'en'))
    await page.reload()
    await page.waitForSelector('ul.scores > li')
    await page.locator('ul.scores > li', { hasText: 'Lacrymosa' }).first().locator('button.open').click()
    await page.waitForSelector('.pdfhost canvas')
    const h = page.locator('.readerhint button.primary')
    if (await h.count()) await h.click()
    await page.waitForTimeout(800)
    const r = await page.evaluate(() => {
      const host = document.querySelector('.pdfhost').getBoundingClientRect()
      const c = document.querySelector('.pdfhost canvas').getBoundingClientRect()
      return { host: host.width, canvas: c.width, ch: c.height, hh: host.height }
    })
    if (r.canvas < r.host - 40) throw new Error(`page ${Math.round(r.canvas)} px in a ${Math.round(r.host)} px pane`)
    if (r.ch <= r.hh) throw new Error('page not taller than the pane: not zoomed to width')
    // Bar 1's system is fully visible when the score opens.
    const sys = await page.evaluate(() => {
      const b = window.__boxes[0]
      const host = document.querySelector('.pdfhost').getBoundingClientRect()
      const c = document.querySelector('.pdfhost canvas').getBoundingClientRect()
      return { top: c.top + b.y * c.height, bottom: c.top + (b.y + b.h) * c.height, vt: host.top, vb: host.bottom }
    })
    if (sys.top < sys.vt - 1 || sys.bottom > sys.vb + 1) throw new Error('system cut: ' + JSON.stringify(sys))
    await shot('50-landscape-width')
    // Following a bar lower on the page scrolls it into view.
    const last = await page.evaluate(() => window.__boxes.reduce((m, b, i) => (b.page === 1 ? i : m), 0))
    await page.evaluate((m) => window.__seekBar?.(m), last)
    await page.waitForTimeout(900)
    const seen = await page.evaluate((m) => {
      const b = window.__boxes[m]
      const host = document.querySelector('.pdfhost').getBoundingClientRect()
      const c = document.querySelector('.pdfhost canvas').getBoundingClientRect()
      const top = c.top + b.y * c.height, bottom = c.top + (b.y + b.h) * c.height
      return top >= host.top - 1 && bottom <= host.bottom + 1
    }, last)
    if (!seen) throw new Error('playing system scrolled out of view')
  })

  await step('perf_06 bar by bar: tapping right and left moves one bar', async () => {
    await page.click('.top button[aria-label="More"]')
    await page.check('label:has-text("Tap moves one bar") input')
    await page.click('.top button:has-text("•••")')
    // Nothing over the page: close the instructions panel and the mixer.
    const ins = page.locator('.top button[aria-label="Instructions"][aria-pressed="true"]')
    if (await ins.count()) await ins.click()
    const mix = page.locator('.top button[aria-label="Hide mixer"]')
    if (await mix.count()) await mix.click()
    await page.waitForTimeout(300)
    const pane = page.locator('.pdfhost')
    const b = await pane.boundingBox()
    const bar = () => page.evaluate(() => window.__playingBar?.() ?? -1)
    await page.evaluate(() => window.__seekBar?.(0))
    await page.waitForTimeout(300)
    for (let i = 0; i < 3; i++) {
      await page.mouse.click(b.x + b.width - 20, b.y + b.height / 2)
      await page.waitForTimeout(250)
    }
    if ((await bar()) !== 3) throw new Error('after 3 right taps bar ' + (await bar()))
    await page.mouse.click(b.x + 20, b.y + b.height / 2)
    await page.waitForTimeout(250)
    if ((await bar()) !== 2) throw new Error('after a left tap bar ' + (await bar()))
    await shot('51-bar-by-bar')
    await page.click('.top button:has-text("•••")')
    await page.uncheck('label:has-text("Tap moves one bar") input')
    await page.click('.top button:has-text("•••")')
  })

  await step('perf_05 drag turns a page like a book: the leading edge folds under the finger', async () => {
    await page.setViewportSize({ width: W, height: H })
    await page.waitForTimeout(600)
    const pane = page.locator('.pdfhost')
    const b = await pane.boundingBox()
    const pg = () => page.locator('.pageno').innerText()
    const before = await pg()
    const cy = b.y + b.height / 2
    // Halfway through a drag the page edge is folded back: the flap spans
    // from the finger to the fold, the lifted part is covered.
    await page.mouse.move(b.x + b.width * 0.9, cy)
    await page.mouse.down()
    for (let i = 1; i <= 8; i++) await page.mouse.move(b.x + b.width * (0.9 - 0.05 * i), cy)
    const mid = await page.evaluate(() => {
      const flap = document.querySelector('.pagefold .pf-flap')?.getBoundingClientRect()
      const cover = document.querySelector('.pagefold .pf-cover')?.getBoundingClientRect()
      const pane = document.querySelector('.pagefold').getBoundingClientRect()
      return flap && cover ? { fl: flap.left, fr: flap.right, cl: cover.left, cr: cover.right, pl: pane.left, pw: pane.width } : null
    })
    if (!mid) throw new Error('no fold while dragging')
    // The page edge started at the pane's right edge and moved with the
    // finger: 0.4 of the pdf pane's width to the left.
    const finger = mid.pl + mid.pw - b.width * 0.4
    if (Math.abs(mid.cr - (mid.pl + mid.pw)) > 2) throw new Error('cover not anchored at the page edge')
    if (Math.abs(mid.fl - finger) > 12) throw new Error(`flap does not start at the finger: ${JSON.stringify(mid)}`)
    if (Math.abs(mid.fr - mid.cl) > 2) throw new Error('flap and cover do not meet at the fold')
    await shot('52-page-fold')
    await page.mouse.up()
    await page.waitForTimeout(600)
    if ((await pg()) === before) throw new Error(`drag past a quarter did not turn (${before})`)
    if (await page.locator('.pagefold').count()) throw new Error('fold left behind')
    // Back: drag to the right.
    await page.mouse.move(b.x + b.width * 0.1, cy)
    await page.mouse.down()
    for (let i = 1; i <= 8; i++) await page.mouse.move(b.x + b.width * (0.1 + 0.07 * i), cy)
    await page.mouse.up()
    await page.waitForTimeout(600)
    if ((await pg()) !== before) throw new Error('drag right did not go back')
    // A short drag falls back without turning.
    await page.mouse.move(b.x + b.width * 0.8, cy)
    await page.mouse.down()
    for (let i = 1; i <= 4; i++) await page.mouse.move(b.x + b.width * (0.8 - 0.02 * i), cy)
    await page.waitForTimeout(200)
    await page.mouse.up()
    await page.waitForTimeout(600)
    if ((await pg()) !== before) throw new Error('a short drag turned the page')
    // A vertical drag does nothing.
    await page.mouse.move(b.x + b.width / 2, b.y + 100)
    await page.mouse.down()
    for (let i = 1; i <= 6; i++) await page.mouse.move(b.x + b.width / 2 + 3 * i, b.y + 100 + 50 * i)
    await page.mouse.up()
    await page.waitForTimeout(400)
    if ((await pg()) !== before) throw new Error('vertical drag turned the page')
  })

  await step('view_08 iPhone library scrolls only vertically (no sideways wobble)', async () => {
    await page.setViewportSize({ width: 393, height: 852 })
    await page.waitForTimeout(400)
    if (await page.locator('.top button[aria-label="Library"]').count()) await page.click('.top button[aria-label="Library"]')
    await page.waitForSelector('ul.scores > li')
    const r = await page.evaluate(() => {
      const l = document.querySelector('.library')
      const cs = getComputedStyle(l)
      l.scrollLeft = 40
      return { ox: cs.overflowX, ta: cs.touchAction, left: l.scrollLeft, sw: l.scrollWidth, cw: l.clientWidth }
    })
    if (r.ox !== 'hidden' || !/pan-y/.test(r.ta) || r.left !== 0) throw new Error('library can move sideways: ' + JSON.stringify(r))
    await page.setViewportSize({ width: W, height: H })
  })

  await step('view_08 iPhone: no control covers the music; practice sheet; header hides while playing; landscape row and auto-hide setting', async () => {
    const overlap = () => page.evaluate(() => {
      // Every visible control vs. the music area (PDF host or notation).
      const m = (document.querySelector('.pdfhost') || document.querySelector('.score'))?.getBoundingClientRect()
      if (!m) return ['no music area']
      const hits = []
      for (const el of document.querySelectorAll('.top, .transport, .minibar, .mixer, .perfbar')) {
        const r = el.getBoundingClientRect()
        const vis = getComputedStyle(el).visibility !== 'hidden' && getComputedStyle(el).display !== 'none' && r.width > 0 && r.height > 0
        if (!vis) continue
        const ix = Math.min(r.right, m.right) - Math.max(r.left, m.left)
        const iy = Math.min(r.bottom, m.bottom) - Math.max(r.top, m.top)
        if (ix > 1 && iy > 1) hits.push(el.className.split(' ')[0] + ' ' + Math.round(iy) + 'px')
      }
      return hits
    })
    // Portrait iPhone 16.
    await page.setViewportSize({ width: 393, height: 852 })
    await page.evaluate(() => localStorage.setItem('cantor.lang', 'en'))
    await page.reload()
    await page.waitForSelector('ul.scores > li')
    await page.locator('ul.scores > li', { hasText: 'Lacrymosa' }).first().locator('button.open').click()
    await page.waitForSelector('.pdfhost canvas')
    await page.waitForTimeout(800)
    const hint = page.locator('.readerhint button.primary')
    if (await hint.count()) await hint.click()
    if (await page.locator('.inspanel').count()) throw new Error('instructions opened over the music on a phone')
    let o = await overlap()
    if (o.length) throw new Error('portrait: controls over the music: ' + o.join(', '))
    const music = await page.locator('.pdfhost').boundingBox()
    if (music.height < 852 * 0.7) throw new Error('portrait: music only ' + Math.round(music.height) + ' px high')
    const canvas = await page.locator('.pdfhost canvas').boundingBox()
    if (canvas.width < music.width - 12) throw new Error('portrait: page not full width')
    if (!(await page.locator('.minibar').isVisible())) throw new Error('no mini player')
    // The header fits the phone: nothing cut off or scrolled sideways.
    const hdr = await page.evaluate(() => { const t = document.querySelector('.top'); return { sw: t.scrollWidth, cw: t.clientWidth } })
    if (hdr.sw > hdr.cw + 1) throw new Error('header overflows: ' + JSON.stringify(hdr))
    // The page view switch is in ••• on a phone.
    await page.click('.top button[aria-label="More"]')
    await page.click('.popover.more .seg button:has-text("Notation")')
    await page.waitForSelector('.score svg g.measure', { timeout: 20000 })
    await page.click('.popover.more .seg button:has-text("Original")').catch(async () => {
      await page.click('.top button[aria-label="More"]')
      await page.click('.popover.more .seg button:has-text("Original")')
    })
    await page.waitForSelector('.pdfhost canvas')
    if (await page.locator('.popover.more').count()) await page.click('.top button[aria-label="More"]')
    await page.waitForTimeout(600)
    await shot('60-iphone-portrait')
    // The practice sheet has the rehearsal controls and the mixer.
    await page.click('.minibar button[aria-label="Practice controls"]')
    await page.waitForSelector('.practicesheet')
    await page.waitForTimeout(400)
    for (const w of ['Tempo', 'Loop', 'Click', 'Tenor 1', 'Practice', 'Performance'])
      if (!(await page.locator('.practicesheet').innerText()).includes(w)) throw new Error('sheet lacks ' + w)
    await shot('61-iphone-sheet')
    await page.click('.practicesheet button[aria-label="Close"]')
    await page.waitForSelector('.practicesheet', { state: 'detached' })
    // Playing hides the header; pausing shows it again.
    await page.click('.minibar button[aria-label="Play"]')
    await page.waitForTimeout(500)
    if (await page.locator('.top').isVisible()) throw new Error('header visible while playing')
    o = await overlap()
    if (o.length) throw new Error('playing: ' + o.join(', '))
    await page.click('.minibar button[aria-label="Pause"]')
    await page.waitForTimeout(300)
    if (!(await page.locator('.top').isVisible())) throw new Error('header not back after pause')
    // Landscape: music only; a tap shows one row below the music.
    await page.setViewportSize({ width: 852, height: 393 })
    await page.waitForTimeout(800)
    if (await page.locator('.minibar').isVisible()) throw new Error('landscape: controls shown before a tap')
    if (await page.locator('.top').isVisible()) throw new Error('landscape: header shown before a tap')
    await shot('62-iphone-landscape')
    const lm = await page.locator('.pdfhost').boundingBox()
    await page.mouse.click(lm.x + lm.width / 2, lm.y + lm.height / 2)
    await page.waitForSelector('.minibar', { state: 'visible' })
    o = await overlap()
    if (o.length) throw new Error('landscape row over the music: ' + o.join(', '))
    await shot('63-iphone-landscape-row')
    // Auto-hide is a setting: "After 4 s" hides the row by itself.
    await page.evaluate(() => localStorage.setItem('choirbird.autohide', '4'))
    await page.mouse.click(lm.x + lm.width / 2, lm.y + lm.height / 2)
    await page.waitForTimeout(200)
    await page.mouse.click(lm.x + lm.width / 2, lm.y + lm.height / 2)
    await page.waitForSelector('.minibar', { state: 'visible' })
    await page.waitForSelector('.minibar', { state: 'hidden', timeout: 6000 })
    await page.evaluate(() => localStorage.removeItem('choirbird.autohide'))
    // The setting is in About on a phone.
    await page.setViewportSize({ width: 393, height: 852 })
    await page.waitForTimeout(400)
    await page.click('.top button[aria-label="Library"]')
    await page.click('header button[aria-label="About ChoirBird"]')
    if (!(await page.locator('.about select[aria-label="Hide controls in landscape"]').count())) throw new Error('auto-hide setting missing')
    await page.click('.about button.primary')
    await page.setViewportSize({ width: W, height: H })
  })

  console.log(errors.length ? 'ERRORS:\n' + errors.join('\n') : 'no page errors')
  await browser.close()
  process.exit(failed || errors.length ? 1 : 0)
})()
