// Raw App Store screenshots: the real UI (mocked backend, marketing mode:
// public-domain titles and the public Lacrymosa fixture only) at the exact
// point sizes of the store's device classes. tools/store/compose.py frames
// them with a headline. Run: tools/ui_harness/run.sh store  (LANG_UI=sv for Swedish)
//   iPhone 6.9" (iPhone 17 Pro Max): 440 x 956 pt @3x = 1320 x 2868 px
//   iPad 13" (iPad Pro M5):          1032 x 1376 pt @2x = 2064 x 2752 px
// The browser has no safe areas, so only the app area is captured (minus the
// status bar and home indicator); compose.py adds those bars back.
// PNGs land in $OUT/shots/store-<lang>-<device>-<name>.png.
const { webkit } = require('playwright')
const path = require('path')
const DIR = __dirname
const LANG = process.env.LANG_UI === 'sv' ? 'sv' : 'en'
const sv = LANG === 'sv'
const out = (dev, n) => path.join(DIR, 'shots', `store-${LANG}-${dev}-${n}.png`)
const DEVICES = {
  // Screen minus status bar (62 pt / 24 pt) and home indicator (34 / 20).
  iphone: { w: 440, h: 956 - 62 - 34, scale: 3 },
  ipad: { w: 1032, h: 1376 - 24 - 20, scale: 2 },
  // Google Play (Android user agent, so the Android wording shows). Screen
  // minus status bar (24 dp) and gesture navigation bar (24 dp).
  //   phone:  412 x 892 dp @2.625 = 1081 x 2342 px (cropped to 1080 x 2340)
  //   tablet: 800 x 1280 dp @2 = 1600 x 2560 px (10-inch)
  aphone: { w: 412, h: 892 - 24 - 24, scale: 2.625, android: true },
  atab: { w: 800, h: 1280 - 24 - 24, scale: 2, android: true },
}
// iPhone-like layout on the Android phone, iPad-like on the tablet.
const like = (dev) => ({ aphone: 'iphone', atab: 'ipad' })[dev] ?? dev

async function open(browser, dev, skin = 'light') {
  const d = DEVICES[dev]
  const ctx = await browser.newContext({ viewport: { width: d.w, height: d.h }, deviceScaleFactor: d.scale, hasTouch: true })
  const page = await ctx.newPage()
  if (d.android)
    await page.addInitScript(() => {
      Object.defineProperty(navigator, 'userAgent', { get: () => 'Mozilla/5.0 (Linux; Android 14; Pixel) AppleWebKit/537.36 Store' })
    })
  await page.addInitScript(([s, l]) => {
    window.__marketing = true
    localStorage.setItem('cantor.skin', s)
    localStorage.setItem('cantor.lang', l)
  }, [skin, LANG])
  await require('./mock.js').setup(page, DIR)
  await page.addInitScript(() => {
    window.__store.hints = ['reader', 'intro', 'start']
  })
  await page.goto(process.env.CANTOR_UI_URL || 'http://localhost:5173/')
  await page.waitForSelector('ul.scores > li')
  await page.waitForSelector('.splash', { state: 'detached', timeout: 8000 })
  const close = page.locator('.getstarted button.secondary, .intro button.secondary')
  if (await close.count()) await close.first().click().catch(() => {})
  return page
}

async function reader(page) {
  await page.locator('ul.scores > li', { hasText: 'Lacrymosa' }).first().locator('button.open').click()
  await page.waitForSelector('.pdfview canvas, .sheet canvas', { timeout: 20000 })
  await page.waitForTimeout(1500)
  const hint = page.locator('.readerhint button.primary')
  if (await hint.count()) await hint.click()
  await page.waitForTimeout(300)
}

// Move the cursor into the music (bar 5 of the score) so the marked bar is
// in the middle of the first system with notes, as while playing.
async function cursorAt(page, sec) {
  await page.evaluate((s) => window.__store && (window.__store.seekTo = s), sec)
  // On a phone the position slider is in the practice sheet.
  let sheet = false
  if (!(await page.locator('input.scrub:visible').count())) {
    const btn = page.locator('.minibar button[aria-label]').last()
    if (await btn.count()) {
      await btn.click()
      await page.waitForTimeout(600)
      sheet = true
    }
  }
  const scrub = page.locator('input.scrub:visible').first()
  if (await scrub.count()) {
    await scrub.evaluate((el, s) => {
      el.value = String(s)
      el.dispatchEvent(new Event('input', { bubbles: true }))
      el.dispatchEvent(new Event('change', { bubbles: true }))
    }, sec)
  }
  if (sheet) {
    const x = page.locator('button[aria-label="Close"], button[aria-label="Stäng"]').last()
    if (await x.count()) await x.click().catch(() => {})
  }
  await page.waitForTimeout(700)
}

async function mixer(page, dev) {
  if (like(dev) === 'ipad') {
    // A wide iPad may open with the mixer already showing.
    if (!(await page.locator('.mixer').isVisible().catch(() => false))) await page.click('button[aria-label="Mixer"]')
  } else {
    const sheet = page.locator('.minibar button[aria-label]').last()
    await sheet.click()
  }
  await page.waitForTimeout(700)
  // "My part" on the alto: tap the name, then the My part preset.
  await page.locator('.strip button.name').nth(1).click().catch(() => {})
  await page.waitForTimeout(200)
  await page.click(`button:has-text("${sv ? 'Min stämma' : 'My part'}")`).catch(() => {})
  await page.waitForTimeout(500)
  // The chosen voice's row, for compose.py's pop-out (pixel copy, not redrawn).
  const row = await page.locator('.strip.mine').first().boundingBox().catch(() => null)
  return row
}

async function closeMixer(page, dev) {
  if (like(dev) === 'ipad') {
    if (await page.locator('.mixer').isVisible().catch(() => false))
      await page.click(`button[aria-label="${sv ? 'Dölj mixer' : 'Hide mixer'}"]`).catch(() => {})
  } else {
    await page.keyboard.press('Escape').catch(() => {})
    const x = page.locator('.sheet button[aria-label="Close"], .sheet button[aria-label="Stäng"]').first()
    if (await x.count()) await x.click().catch(() => {})
  }
  await page.waitForTimeout(400)
}

// On a phone, view and annotate live in the ••• menu.
async function headerAction(page, label) {
  const direct = page.locator(`.top > * button:has-text("${label}"), .top button:has-text("${label}")`).first()
  if (await direct.isVisible().catch(() => false)) return direct.click()
  await page.click('.top button:has-text("•••")')
  await page.click(`.popover.more button:has-text("${label}")`)
  if (await page.locator('.popover.more').isVisible().catch(() => false)) await page.click('.top button:has-text("•••")')
}

// Page view (••• menu): 'scroll' fills a phone screen with music.
async function pageMode(page, m) {
  await page.click('.top button:has-text("•••")')
  await page.selectOption('.popover.more select >> nth=-1', m).catch(() => {})
  await page.click('.top button:has-text("•••")')
  await page.waitForTimeout(900)
}

async function annotate(page) {
  await headerAction(page, sv ? 'Anteckna' : 'Annotate')
  await page.waitForSelector('.annbar')
  const svg = page.locator('svg.annot').first()
  const b = await svg.boundingBox()
  const boxes = await page.evaluate(() => window.__boxes ?? [])
  // A highlighter over the alto line of bar 5 and a pen breath mark.
  const m = boxes[4] ?? boxes[0]
  await page.click(`.annbar button:has-text("${sv ? 'Överstrykning' : 'Highlighter'}")`).catch(() => {})
  if (m) {
    const x0 = b.x + b.width * (m.x + m.w * 0.05)
    const x1 = b.x + b.width * (m.x + m.w * 0.95)
    const y = b.y + b.height * (m.y + m.h * 0.3)
    await page.mouse.move(x0, y)
    await page.mouse.down()
    for (let i = 1; i <= 20; i++) await page.mouse.move(x0 + ((x1 - x0) * i) / 20, y)
    await page.mouse.up()
  }
  await page.click(`.annbar button:has-text("${sv ? 'Stämpel' : 'Stamp'}")`)
  await page.click('.annbar .stamps button:has-text("mf")')
  if (m) await page.mouse.click(b.x + b.width * (m.x + m.w * 0.15), b.y + b.height * (m.y + m.h * 0.62))
  await page.waitForTimeout(500)
}

;(async () => {
  const browser = await webkit.launch()
  const only = process.env.STORE_DEVICE
  const devs = process.env.STORE_PLAY ? ['aphone', 'atab'] : ['iphone', 'ipad']
  for (const dev of devs) {
    if (only && only !== dev) continue
    // 1. Hero: the original PDF with the playing bar marked, practice controls.
    {
      const page = await open(browser, dev)
      await reader(page)
      if (like(dev) === 'iphone') await pageMode(page, 'scroll')
      else await closeMixer(page, dev)
      await cursorAt(page, 9)
      await page.screenshot({ path: out(dev, '1-score') })
      // 2. Mixer: every voice on its own line, My part on the alto.
      const row = await mixer(page, dev)
      await page.screenshot({ path: out(dev, '2-mixer') })
      if (row) {
        const s = DEVICES[dev].scale
        require('fs').writeFileSync(out(dev, '2-mixer').replace(/\.png$/, '.json'), JSON.stringify({ popout: [row.x * s, row.y * s, row.width * s, row.height * s] }))
      }
      await page.context().close()
    }
    // 3. Notation view (re-engraved, transposable).
    {
      const page = await open(browser, dev)
      await reader(page)
      await headerAction(page, sv ? 'Notbild' : 'Notation').catch(() => {})
      await page.waitForSelector('.score svg', { timeout: 20000 }).catch(() => {})
      if (like(dev) === 'ipad') await closeMixer(page, dev)
      await cursorAt(page, 9)
      await page.waitForTimeout(1200)
      await page.screenshot({ path: out(dev, '3-notation') })
      await page.context().close()
    }
    // 4. Annotations on the PDF.
    {
      const page = await open(browser, dev)
      await reader(page)
      if (like(dev) === 'ipad') await closeMixer(page, dev)
      await annotate(page)
      await page.screenshot({ path: out(dev, '4-annotate') })
      await page.context().close()
    }
    // 5. Library: the whole repertoire, with setlists as tabs.
    {
      const page = await open(browser, dev)
      await page.waitForTimeout(600)
      await page.screenshot({ path: out(dev, '5-library') })
      await page.context().close()
    }
    // 6. Dark reading at night.
    {
      const page = await open(browser, dev, 'dark')
      await reader(page)
      if (like(dev) === 'ipad') await closeMixer(page, dev)
      await cursorAt(page, 9)
      await page.screenshot({ path: out(dev, '6-dark') })
      await page.context().close()
    }
  }
  await browser.close()
  console.log('store shots done')
})()
