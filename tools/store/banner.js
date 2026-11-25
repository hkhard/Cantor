// App Store creative assets for ChoirBird (product page header, search
// results), rendered in WebKit from HTML so the type and the brand mark are
// crisp vector art. Specs (App Store Connect, creative assets):
//   universal  5244 x 2950 PNG, art safe area 1402 x 962 at (1921, 660)
//   header     3840 x 1646 PNG, art safe area 1646 x 661 at (1097, 493)
// Key art (mark, name, one line) stays inside the safe area; real app
// screens and staff lines fill the auxiliary area around it.
//   node tools/store/banner.js RAW_SHOTS_DIR OUT_DIR   (run by make_banners.sh)
const { webkit } = require(process.env.PW_DIR ? require('path').join(process.env.PW_DIR, 'node_modules/playwright') : 'playwright')
const fs = require('fs')
const path = require('path')

const [rawDir, outDir] = process.argv.slice(2)
const ROOT = path.resolve(__dirname, '../..')
const mark = fs.readFileSync(path.join(ROOT, 'app/ui/public/choirbird-mark.svg'), 'utf8')
const img = (f) => 'data:image/png;base64,' + fs.readFileSync(path.join(rawDir, f)).toString('base64')

const TEXT = {
  en: { line: 'Hear your part. Sing it better.', sub: 'Sheet music that plays every part' },
  sv: { line: 'Hör din stämma. Sjung säkrare.', sub: 'Noter som spelar alla stämmor' },
}
const SPECS = {
  universal: { w: 5244, h: 2950, safe: { x: 1921, y: 660, w: 1402, h: 962 } },
  header: { w: 3840, h: 1646, safe: { x: 1097, y: 493, w: 1646, h: 661 } },
}

function html(lang, spec, debug) {
  const { w, h, safe } = spec
  const t = TEXT[lang]
  // Device sizes relative to canvas height; devices sit left and right of
  // the safe area, partly cropped by the canvas edge (auxiliary art).
  const ph = Math.round(h * 0.86) // iPad height
  const pw = Math.round(ph * 1032 / 1332)
  const fh = Math.round(h * 0.80) // iPhone height
  const fw = Math.round(fh * 440 / 860)
  const gap = Math.round(h * 0.05)
  const ipadX = safe.x - gap - pw
  const phoneX = safe.x + safe.w + gap
  const markSize = Math.round(safe.h * 0.42)
  const nameSize = Math.round(safe.h * 0.205)
  const lineSize = Math.round(safe.h * 0.088)
  const ipad = img(`store-${lang}-ipad-2-mixer.png`)
  const phone = img(`store-${lang}-iphone-1-score.png`)
  // Staff lines across the whole canvas, like a page of music behind glass.
  const staves = [0.16, 0.5, 0.84].map((cy) =>
    [0, 1, 2, 3, 4].map((i) => `<div class="line" style="top:${Math.round(h * cy + (i - 2) * h * 0.022)}px"></div>`).join(''),
  ).join('')
  return `<!doctype html><html><head><meta charset="utf-8"><style>
  html,body{margin:0;width:${w}px;height:${h}px;overflow:hidden}
  body{font-family:-apple-system,'SF Pro Display',system-ui,sans-serif;
    background:radial-gradient(60% 75% at 50% 48%, #2A4C96 0%, #1E3A7B 38%, #0D1B3E 78%, #0A1430 100%)}
  .line{position:absolute;left:0;right:0;height:${Math.max(3, Math.round(h * 0.0016))}px;background:rgba(244,235,221,.07)}
  .dev{position:absolute;overflow:hidden;background:#0C0C0E;box-shadow:0 ${Math.round(h*0.02)}px ${Math.round(h*0.06)}px rgba(0,0,0,.45), 0 0 0 ${Math.max(4,Math.round(h*0.003))}px #3A3E4A}
  .dev img{position:absolute;display:block}
  .ipad{left:${ipadX}px;top:${Math.round((h - ph) / 2)}px;width:${pw}px;height:${ph}px;border-radius:${Math.round(pw*0.055)}px;transform:rotate(-4deg)}
  .ipad img{left:${Math.round(pw*0.028)}px;top:${Math.round(pw*0.028)}px;width:${pw - 2*Math.round(pw*0.028)}px;border-radius:${Math.round(pw*0.03)}px}
  .phone{left:${phoneX}px;top:${Math.round((h - fh) / 2 + h*0.04)}px;width:${fw}px;height:${fh}px;border-radius:${Math.round(fw*0.17)}px;transform:rotate(5deg)}
  .phone img{left:${Math.round(fw*0.035)}px;top:${Math.round(fw*0.035) + Math.round(fw*0.16)}px;width:${fw - 2*Math.round(fw*0.035)}px}
  .phone .bar{position:absolute;left:${Math.round(fw*0.035)}px;right:${Math.round(fw*0.035)}px;top:${Math.round(fw*0.035)}px;height:${Math.round(fw*0.16)}px;background:#FBF6EF;border-radius:${Math.round(fw*0.135)}px ${Math.round(fw*0.135)}px 0 0}
  .phone .island{position:absolute;left:50%;top:${Math.round(fw*0.065)}px;width:${Math.round(fw*0.3)}px;height:${Math.round(fw*0.085)}px;margin-left:-${Math.round(fw*0.15)}px;border-radius:999px;background:#000}
  .safe{position:absolute;left:${safe.x}px;top:${safe.y}px;width:${safe.w}px;height:${safe.h}px;
    display:flex;flex-direction:column;align-items:center;justify-content:center;text-align:center;${debug ? 'outline:6px dashed #0f0;' : ''}}
  .safe svg{width:${markSize}px;height:${markSize}px;filter:drop-shadow(0 ${Math.round(markSize*0.05)}px ${Math.round(markSize*0.12)}px rgba(0,0,0,.45))}
  .name{margin-top:${Math.round(safe.h*0.035)}px;font-weight:800;font-size:${nameSize}px;line-height:1;letter-spacing:-0.02em;color:#FFF6EC;font-family:'New York','Iowan Old Style',Georgia,serif}
  .tag{margin-top:${Math.round(safe.h*0.04)}px;font-weight:700;font-size:${lineSize}px;line-height:1.1;color:#FF7A5C;white-space:nowrap}
  .glow{position:absolute;left:${safe.x - safe.w*0.15}px;top:${safe.y - safe.h*0.15}px;width:${safe.w*1.3}px;height:${safe.h*1.3}px;border-radius:50%;background:radial-gradient(closest-side, rgba(13,27,62,.85), rgba(13,27,62,0))}
  </style></head><body>
  ${staves}
  <div class="dev ipad"><img src="${ipad}"></div>
  <div class="dev phone"><div class="bar"></div><img src="${phone}"><div class="island"></div></div>
  <div class="glow"></div>
  <div class="safe">${mark}<div class="name">ChoirBird</div><div class="tag">${t.line}</div></div>
  </body></html>`
}

;(async () => {
  fs.mkdirSync(outDir, { recursive: true })
  const browser = await webkit.launch()
  for (const lang of ['en', 'sv']) {
    for (const [name, spec] of Object.entries(SPECS)) {
      for (const debug of [false, true]) {
        const page = await browser.newPage({ viewport: { width: spec.w, height: spec.h }, deviceScaleFactor: 1 })
        await page.setContent(html(lang, spec, debug))
        await page.waitForTimeout(400)
        // The tagline must fit inside the safe area (a hard check, not a guess).
        const fit = await page.evaluate(() => {
          const s = document.querySelector('.safe').getBoundingClientRect()
          return [...document.querySelectorAll('.safe > *')].every((e) => {
            const r = e.getBoundingClientRect()
            return r.left >= s.left - 1 && r.right <= s.right + 1 && r.top >= s.top - 1 && r.bottom <= s.bottom + 1
          })
        })
        if (!fit) throw new Error(`${lang} ${name}: key art outside the safe area`)
        const file = path.join(outDir, `${name}-${lang}${debug ? '-safearea' : ''}.png`)
        await page.screenshot({ path: file, omitBackground: false })
        await page.close()
        console.log(file)
      }
    }
  }
  await browser.close()
})()
