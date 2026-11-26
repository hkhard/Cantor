// NFR-08 accessibility audit: axe-core (WCAG 2.0/2.1/2.2 A and AA rules) on
// every screen of the real UI, in each skin (light, dark, sepia) and both
// languages, at iPad mini size. Also checks that every control has an
// accessible name, that keyboard focus is visible, that text still fits at
// 200% text size, and that reduced motion turns animations off.
// Exit code 1 when anything fails.
const { webkit } = require('playwright')
const fs = require('fs')
const path = require('path')
const DIR = __dirname
const AXE = fs.readFileSync(require.resolve('axe-core/axe.min.js'), 'utf8')
const W = Number(process.env.W || 744)
const H = Number(process.env.H || 1133)

const SCREENS = [
  ['library', async () => {}],
  // LIB-11 layouts: setlists as rows, and the columns (landscape: three).
  ['library-lists', async (p) => {
    await p.locator('.layoutsel button').nth(1).click()
    await p.waitForSelector('.rows')
  }],
  ['library-columns', async (p) => {
    await p.setViewportSize({ width: H, height: W })
    await p.locator('.layoutsel button').nth(2).click()
    await p.waitForSelector('.pane.content')
    await p.locator('.pane.lists .rows > li button').first().click().catch(() => {})
  }],
  ['metadata', async (p) => {
    await p.locator('ul.scores > li').first().locator('button.info').click()
    await p.waitForSelector('.metasheet')
  }],
  ['about', async (p) => {
    await p.click('header button.aboutpill')
    await p.waitForSelector('.about')
  }],
  ['setlist', async (p) => {
    await p.click('.tabs > button.chip')
    await p.waitForSelector('.dlg input')
    await p.fill('.dlg input', 'A11y')
    await p.click('.dlg button.primary')
    await p.waitForSelector('h2.setname')
  }],
  ['dialog', async (p) => {
    await p.click('.tabs > button.chip')
    await p.waitForSelector('.dlg')
  }],
  ['reader-pdf', async (p) => {
    await p.locator('ul.scores > li').first().locator('button.open').click()
    await p.waitForSelector('.pdfhost canvas')
    await p.waitForTimeout(1200)
  }],
  ['reader-more', async (p) => {
    await p.locator('ul.scores > li').first().locator('button.open').click()
    await p.waitForSelector('.pdfhost canvas')
    await p.click('.top button[aria-haspopup="dialog"]')
    await p.waitForSelector('.popover.more')
  }],
  ['reader-mixer', async (p) => {
    await p.locator('ul.scores > li').first().locator('button.open').click()
    await p.waitForSelector('.pdfhost canvas')
    const mix = p.locator('.top button:has(svg)').last()
    if (!(await p.locator('aside.mixer').count())) await mix.click()
    await p.waitForSelector('aside.mixer')
    await p.click('.transport .exportwrap button')
  }],
  ['reader-annotate', async (p) => {
    await p.locator('ul.scores > li').first().locator('button.open').click()
    await p.waitForSelector('.pdfhost canvas')
    await p.click('.top button.annotate')
    await p.waitForSelector('.annbar')
    await p.locator('.annbar .seg button').nth(4).click()
  }],
  ['reader-notation', async (p) => {
    await p.locator('ul.scores > li').first().locator('button.open').click()
    await p.waitForSelector('.pdfhost canvas')
    await p.locator('.top .seg button').nth(1).click()
    await p.waitForSelector('.score svg g.measure', { timeout: 20000 })
  }],
  ['reader-pdfonly', async (p) => {
    await p.locator('ul.scores > li', { hasText: 'Min egen' }).locator('button.open').click()
    await p.waitForSelector('.transport.silent')
  }],
  ['performance', async (p) => {
    await p.locator('ul.scores > li').first().locator('button.open').click()
    await p.waitForSelector('.pdfhost canvas')
    await p.click('.modeswitch button:nth-child(2)')
    await p.waitForSelector('.perfexit')
  }],
  // SET-03/04: instructions panel (with hints and a practice track) and a
  // received setlist with its instructions.
  ['reader-instructions', async (p) => {
    await p.evaluate(() => {
      window.__store.instructions = { '1:Lacrymosa': { text: 'Andas före t. 3. **Piano** se [noter](https://example.org).', parts: [{ who: 'Bo', part: 'Tenor 1' }] } }
      window.__store.member = 'Bo'
      window.__store.practice = { '1:Lacrymosa': ['Tenor 1', 'Bas 1'] }
    })
    await p.locator('ul.scores > li').first().locator('button.open').click()
    await p.waitForSelector('.inspanel .barref')
  }],
  ['reader-instructions-edit', async (p) => {
    await p.locator('ul.scores > li').first().locator('button.open').click()
    await p.waitForSelector('.pdfhost canvas')
    await p.click('.top button[aria-pressed][aria-label]:has-text("ℹ")')
    await p.click('.inspanel button.chip:not([aria-label])')
    await p.waitForSelector('.inspanel textarea')
  }],
  ['setlist-received', async (p) => {
    await p.evaluate(() => {
      window.__store.setlists = [{ id: 'rx', name: 'Vår', instructions: 'Samling **17.30**.', items: [{ kind: 'score', score: '1:Lacrymosa', start_page: null, note: 'Börja i t. 9' }] }]
      window.__store.receivedSet = 'rx'
    })
    await p.reload()
    await p.waitForSelector('.setins')
  }],
  // Phase P first run: empty library and the one-time reader hint.
  ['first-run', async (p) => {
    await p.evaluate(() => { window.__store.hints = []; window.__removeScore('1:Mine', true); window.__removeScore('1:Scanned', true) })
    await p.reload()
    await p.waitForSelector('.welcome .trycard')
  }],
  ['reader-hint', async (p) => {
    await p.evaluate(() => { window.__store.hints = [] })
    await p.locator('ul.scores > li').first().locator('button.open').click()
    await p.waitForSelector('.readerhint')
  }],
]

;(async () => {
  const only = process.env.ONLY
  const browser = await webkit.launch()
  let failures = 0
  const report = []
  for (const skin of ['light', 'dark', 'sepia']) {
    for (const lang of ['sv', 'en']) {
      for (const [name, go] of SCREENS) {
        if (only && !name.includes(only)) continue
        const ctx = await browser.newContext({ viewport: { width: W, height: H }, deviceScaleFactor: 1, hasTouch: true })
        const page = await ctx.newPage()
        await require('./mock.js').setup(page, DIR)
        await page.addInitScript(([skin, lang]) => {
          localStorage.setItem('cantor.skin', skin)
          localStorage.setItem('cantor.lang', lang)
        }, [skin, lang])
        await page.goto((process.env.CANTOR_UI_URL || 'http://localhost:5173/'))
        await page.waitForSelector('ul.scores > li')
    await page.waitForSelector('.splash', { state: 'detached', timeout: 5000 })
        try {
          await go(page)
        } catch (e) {
          failures++
          report.push(`${skin}/${lang}/${name}: could not reach screen: ${e.message.split('\n')[0]}`)
          await ctx.close()
          continue
        }
        await page.waitForTimeout(400)
        await page.addScriptTag({ content: AXE })
        const res = await page.evaluate(async () => {
          const r = await window.axe.run(document, {
            runOnly: { type: 'tag', values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'] },
          })
          return r.violations.map((v) => ({
            id: v.id,
            impact: v.impact,
            help: v.help,
            nodes: v.nodes.slice(0, 4).map((n) => ({ target: n.target.join(' '), summary: (n.failureSummary || '').split('\n').slice(0, 3).join(' | ') })),
            count: v.nodes.length,
          }))
        })
        // Every interactive control has an accessible name.
        const unnamed = await page.evaluate(() => {
          const out = []
          for (const el of document.querySelectorAll('button, input, select, textarea, [role=button], [role=tab], [role=menuitem]')) {
            if (el.closest('[aria-hidden=true]') || el.offsetParent === null) continue
            const name =
              el.getAttribute('aria-label') ||
              (el.labels && [...el.labels].map((l) => l.textContent).join(' ')) ||
              el.getAttribute('title') ||
              el.getAttribute('placeholder') ||
              el.textContent
            if (!name || !name.trim()) out.push(el.outerHTML.slice(0, 120))
          }
          return out
        })
        const tag = `${skin}/${lang}/${name}`
        if (res.length || unnamed.length) {
          failures += res.length + unnamed.length
          for (const v of res) {
            report.push(`${tag}: [${v.impact}] ${v.id} (${v.count}) ${v.help}`)
            for (const n of v.nodes) report.push(`    ${n.target} :: ${n.summary}`)
          }
          for (const u of unnamed) report.push(`${tag}: unnamed control ${u}`)
        }
        await ctx.close()
      }
    }
  }

  // Keyboard: Tab reaches the library controls and focus is visible.
  {
    const ctx = await browser.newContext({ viewport: { width: W, height: H } })
    const page = await ctx.newPage()
    await require('./mock.js').setup(page, DIR)
    await page.goto((process.env.CANTOR_UI_URL || 'http://localhost:5173/'))
    await page.waitForSelector('ul.scores > li')
    await page.waitForSelector('.splash', { state: 'detached', timeout: 5000 })
    const seen = []
    for (let i = 0; i < 25; i++) {
      await page.keyboard.press('Tab')
      const f = await page.evaluate(() => {
        const el = document.activeElement
        if (!el || el === document.body) return null
        const cs = getComputedStyle(el)
        const visible =
          (cs.outlineStyle !== 'none' && parseFloat(cs.outlineWidth) > 0) || cs.boxShadow !== 'none'
        return { tag: el.tagName, text: (el.getAttribute('aria-label') || el.textContent || '').trim().slice(0, 30), visible }
      })
      if (f) seen.push(f)
    }
    const invisible = seen.filter((f) => !f.visible)
    if (seen.length < 8) {
      failures++
      report.push(`keyboard: only ${seen.length} focus stops on the library`)
    }
    if (invisible.length) {
      failures++
      report.push(`keyboard: focus not visible on ${invisible.map((f) => f.tag + ' ' + f.text).join(', ')}`)
    }
    // Space plays, arrows turn pages in the reader (pedal keys).
    await page.locator('ul.scores > li').first().locator('button.open').click()
    await page.waitForSelector('.pdfhost canvas')
    await page.locator('.pdfhost').click({ position: { x: 5, y: 5 } }).catch(() => {})
    await page.keyboard.press('Space')
    await page.waitForTimeout(200)
    const played = await page.evaluate(() => window.__calls.some((c) => c[0] === 'play'))
    if (!played) {
      failures++
      report.push('keyboard: Space did not start playback in the reader')
    }
    await ctx.close()
  }

  // Large text (200%): the reader's controls must not overflow off screen.
  {
    const ctx = await browser.newContext({ viewport: { width: W, height: H } })
    const page = await ctx.newPage()
    await require('./mock.js').setup(page, DIR)
    await page.addInitScript(() => localStorage.setItem('cantor.textScale', '2'))
    await page.goto((process.env.CANTOR_UI_URL || 'http://localhost:5173/'))
    await page.waitForSelector('ul.scores > li')
    await page.waitForSelector('.splash', { state: 'detached', timeout: 5000 })
    for (const screen of ['library', 'reader']) {
      if (screen === 'reader') {
        await page.locator('ul.scores > li').first().locator('button.open').click()
        await page.waitForSelector('.pdfhost canvas')
      }
      await page.waitForTimeout(400)
      const over = await page.evaluate(() => {
        const out = []
        const vw = document.documentElement.clientWidth
        for (const el of document.querySelectorAll('button, input, select, label, h1, h2, p')) {
          if (el.offsetParent === null) continue
          const r = el.getBoundingClientRect()
          if (r.right > vw + 1 || r.left < -1) out.push(el.tagName + ' ' + (el.textContent || '').trim().slice(0, 25))
        }
        return out
      })
      await page.screenshot({ path: path.join(DIR, 'shots', `a11y-bigtext-${screen}.png`) })
      if (over.length) {
        failures++
        report.push(`text 200% ${screen}: off-screen ${over.slice(0, 8).join('; ')}`)
      }
    }
    await ctx.close()
  }

  // Reduced motion: no transitions/animations longer than 10 ms.
  {
    const ctx = await browser.newContext({ viewport: { width: W, height: H }, reducedMotion: 'reduce' })
    const page = await ctx.newPage()
    await require('./mock.js').setup(page, DIR)
    await page.goto((process.env.CANTOR_UI_URL || 'http://localhost:5173/'))
    await page.waitForSelector('ul.scores > li')
    await page.waitForSelector('.splash', { state: 'detached', timeout: 5000 })
    const moving = await page.evaluate(() => {
      const out = []
      for (const el of document.querySelectorAll('*')) {
        const cs = getComputedStyle(el)
        const t = Math.max(...cs.transitionDuration.split(',').map((x) => parseFloat(x) || 0))
        const a = Math.max(...cs.animationDuration.split(',').map((x) => parseFloat(x) || 0))
        if (t > 0.01 || a > 0.01) out.push(el.tagName + '.' + el.className)
      }
      return out
    })
    if (moving.length) {
      failures++
      report.push(`reduced motion: still animating ${[...new Set(moving)].slice(0, 6).join(', ')}`)
    }
    await ctx.close()
  }

  await browser.close()
  console.log(report.length ? report.join('\n') : 'no accessibility findings')
  console.log(`a11y findings: ${failures}`)
  process.exit(failures ? 1 : 0)
})()
