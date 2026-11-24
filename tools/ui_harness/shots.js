// Marketing screenshots for the ChoirBird web page (nikielhard.se/en/choirbird).
// The real UI with the mocked backend in marketing mode (window.__marketing):
// only public-domain titles, and the public Lacrymosa fixture with its real
// SATB + piano parts. Run through tools/ui_harness/run.sh shots; the PNGs land
// in $OUT/shots/marketing-*.png.
const { webkit } = require('playwright')
const path = require('path')
const DIR = __dirname
const out = (n) => path.join(DIR, 'shots', `marketing-${n}.png`)

async function open(browser, { w, h, skin = 'light' }) {
  const ctx = await browser.newContext({ viewport: { width: w, height: h }, deviceScaleFactor: 2, hasTouch: true })
  const page = await ctx.newPage()
  await page.addInitScript((s) => {
    window.__marketing = true
    localStorage.setItem('cantor.skin', s)
    localStorage.setItem('cantor.lang', 'en')
  }, skin)
  await require('./mock.js').setup(page, DIR)
  // Getting started is for new users; the marketing library is a lived-in one.
  await page.addInitScript(() => {
    window.__store.hints = ['reader', 'intro', 'start']
  })
  await page.goto(process.env.CANTOR_UI_URL || 'http://localhost:5173/')
  await page.waitForSelector('ul.scores > li')
  await page.waitForSelector('.splash', { state: 'detached', timeout: 8000 })
  const close = page.locator('.getstarted button:has-text("Close"), .intro button:has-text("Close")')
  if (await close.count()) await close.first().click()
  return page
}

async function reader(page) {
  await page.locator('ul.scores > li', { hasText: 'Lacrymosa' }).first().locator('button.open').click()
  await page.waitForSelector('.pdfview canvas, .sheet canvas', { timeout: 20000 })
  await page.waitForTimeout(1500)
  const hint = page.locator('.readerhint button.primary')
  if (await hint.count()) await hint.click()
}

;(async () => {
  const browser = await webkit.launch()
  // iPad mini, portrait: library, PDF with the cursor and mixer, notation.
  for (const skin of ['light', 'dark', 'sepia']) {
    const page = await open(browser, { w: 744, h: 1133, skin })
    if (skin === 'light') await page.screenshot({ path: out('ipad-library') })
    await reader(page)
    await page.screenshot({ path: out(`ipad-pdf-${skin}`) })
    if (skin === 'light' || skin === 'dark') {
      // The mixer: every voice its own line, "My part" on the alto.
      await page.click('button[aria-label="Mixer"]')
      await page.waitForTimeout(800)
      const alto = page.locator('aside button', { hasText: 'Alto' }).first()
      if (await alto.count()) await alto.click().catch(() => {})
      await page.waitForTimeout(600)
      await page.screenshot({ path: out(skin === 'dark' ? 'ipad-mixer-dark' : 'ipad-mixer') })
      await page.click('button[aria-label="Hide mixer"]').catch(() => {})
      await page.waitForTimeout(400)
    }
    if (skin === 'light') {
      await page.click('button:has-text("Notation")')
      await page.waitForSelector('.score svg', { timeout: 20000 })
      await page.waitForTimeout(1200)
      await page.screenshot({ path: out('ipad-notation') })
    }
    await page.context().close()
  }
  // iPad in landscape: the page fills the width.
  {
    const page = await open(browser, { w: 1133, h: 744 })
    await reader(page)
    await page.screenshot({ path: out('ipad-landscape') })
    await page.context().close()
  }
  // iPhone, portrait, dark: for the web page's dark appearance.
  {
    const page = await open(browser, { w: 393, h: 852, skin: 'dark' })
    await reader(page)
    await page.screenshot({ path: out('iphone-reader-dark') })
    await page.context().close()
  }
  // iPhone, portrait: nothing covers the music.
  {
    const page = await open(browser, { w: 393, h: 852 })
    await page.screenshot({ path: out('iphone-library') })
    await reader(page)
    await page.screenshot({ path: out('iphone-reader') })
    const sheet = page.locator('.minibar button[aria-label]').last()
    if (await sheet.count()) {
      await sheet.click()
      await page.waitForTimeout(600)
      await page.screenshot({ path: out('iphone-mixer') })
    }
    await page.context().close()
  }
  await browser.close()
  console.log('marketing shots done')
})()
