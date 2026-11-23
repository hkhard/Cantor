// VIEW-06: screenshots of every skin on the screens where reading matters
// (library, PDF page, engraved notation, mixer), plus measured contrast of
// the page itself: paper against ink, and on a rendered PDF the darkest
// pixels (notes) against the page colour.
//   ONLY=<skin> node skins.js   (run through tools/ui_harness/run.sh skins)
const { webkit } = require('playwright')
const path = require('path')
const DIR = __dirname
const W = Number(process.env.W || 744)
const H = Number(process.env.H || 1133)

function lum([r, g, b]) {
  const f = (c) => ((c /= 255) <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4)
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b)
}
const ratio = (a, b) => {
  const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p)
  return (x + 0.05) / (y + 0.05)
}

;(async () => {
  const browser = await webkit.launch()
  let bad = 0
  for (const skin of ['light', 'dark', 'sepia']) {
    if (process.env.ONLY && process.env.ONLY !== skin) continue
    const ctx = await browser.newContext({ viewport: { width: W, height: H }, deviceScaleFactor: 2, hasTouch: true })
    const page = await ctx.newPage()
    await require('./mock.js').setup(page, DIR)
    await page.addInitScript((s) => {
      localStorage.setItem('cantor.skin', s)
      localStorage.setItem('cantor.lang', 'en')
    }, skin)
    await page.goto(process.env.CANTOR_UI_URL || 'http://localhost:5173/')
    await page.waitForSelector('ul.scores > li')
    await page.waitForSelector('.splash', { state: 'detached', timeout: 5000 })
    await page.screenshot({ path: path.join(DIR, 'shots', `skin-${skin}-library.png`) })
    await page.locator('ul.scores > li', { hasText: 'Lacrymosa' }).first().locator('button.open').click()
    await page.waitForSelector('.pdfview canvas, .sheet canvas', { timeout: 20000 })
    await page.waitForTimeout(1500)
    const hint = page.locator('.readerhint button.primary')
    if (await hint.count()) await hint.click()
    await page.screenshot({ path: path.join(DIR, 'shots', `skin-${skin}-pdf.png`) })
    // Page vs. notes on the rendered PDF canvas: lightest and darkest pixels.
    const pdf = await page.evaluate(() => {
      const c = document.querySelector('.sheet canvas')
      if (!c) return null
      const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data
      // The page colour is the most common one; the notes are the pixels
      // furthest from it.
      const count = new Map()
      for (let i = 0; i < d.length; i += 64) {
        const k = `${d[i]},${d[i + 1]},${d[i + 2]}`
        count.set(k, (count.get(k) ?? 0) + 1)
      }
      const page = [...count.entries()].sort((a, b) => b[1] - a[1])[0][0].split(',').map(Number)
      let ink = page, far = -1
      for (let i = 0; i < d.length; i += 16) {
        const dist = Math.abs(d[i] - page[0]) + Math.abs(d[i + 1] - page[1]) + Math.abs(d[i + 2] - page[2])
        if (dist > far) (far = dist), (ink = [d[i], d[i + 1], d[i + 2]])
      }
      return { page, ink }
    })
    await page.click('.seg button:has-text("Notation")')
    await page.waitForSelector('.score svg g.measure', { timeout: 20000 })
    await page.waitForTimeout(800)
    await page.screenshot({ path: path.join(DIR, 'shots', `skin-${skin}-notation.png`) })
    const nota = await page.evaluate(() => {
      const s = document.querySelector('.score')
      const cs = getComputedStyle(s)
      const rgb = (v) => (v.match(/\d+(\.\d+)?/g) || []).slice(0, 3).map(Number)
      // A note head outside the playing bar: what the singer reads.
      const head = document.querySelector('.score g.measure:not(.playing) g.notehead use')
      return { paper: rgb(cs.backgroundColor), ink: rgb(getComputedStyle(head ?? s).fill) }
    })
    await page.click('.top button[aria-label="Mixer"], .top button:has-text("Mixer")').catch(() => {})
    await page.waitForTimeout(400)
    await page.screenshot({ path: path.join(DIR, 'shots', `skin-${skin}-mixer.png`) })
    const r1 = pdf ? ratio(pdf.page, pdf.ink) : 0
    const r2 = ratio(nota.paper, nota.ink)
    console.log(
      `${skin}: PDF page rgb(${pdf?.page}) notes rgb(${pdf?.ink}) contrast ${r1.toFixed(1)}; notation paper rgb(${nota.paper}) ink rgb(${nota.ink}) contrast ${r2.toFixed(1)}`,
    )
    // Reading music: at least 7:1 (WCAG AAA for text) on both views, and the
    // paper is one of the three: white, sepia or black.
    if (r1 < 7 || r2 < 7) bad++
    await ctx.close()
  }
  await browser.close()
  console.log(bad ? `skins: ${bad} skin(s) below 7:1 on the page` : 'skins: every page view >= 7:1')
  process.exit(bad ? 1 : 0)
})()
