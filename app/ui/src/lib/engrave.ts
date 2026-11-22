// Verovio engraving: MusicXML -> SVG pages, and written measure -> page/element
// lookup for the play cursor.
import createVerovioModule from 'verovio/wasm'
import { VerovioToolkit } from 'verovio/esm'

let toolkitPromise: Promise<VerovioToolkit> | null = null

function toolkit(): Promise<VerovioToolkit> {
  if (!toolkitPromise) {
    toolkitPromise = createVerovioModule().then((m) => new VerovioToolkit(m as never))
  }
  return toolkitPromise
}

export interface Engraved {
  pages: number
  /** written measure index -> { page, id } */
  measurePage: Map<number, { page: number; id: string }>
  render(page: number): string
}

/** Verovio scale for a pane width in CSS pixels. Verovio's page units are
 *  1/10 mm at scale 100, so a smaller scale fits more music per line.
 *  Tuned on a choir score (2 systems x 2 staves, lyrics): about 5 bars per
 *  line on an iPad mini in portrait (744 px) and 6 on an 11-13" iPad, with
 *  noteheads still about 2 mm on the glass. */
export function scaleFor(width: number): number {
  if (width < 560) return 28
  if (width < 820) return 34
  if (width < 1100) return 40
  return 45
}

export async function engrave(
  musicxml: string,
  width: number,
  height: number,
  zoom = 1,
): Promise<Engraved> {
  const tk = await toolkit()
  const scale = Math.round(scaleFor(width) * zoom)
  tk.setOptions({
    pageWidth: Math.round((width * 100) / scale),
    pageHeight: Math.round((height * 100) / scale),
    scale,
    adjustPageHeight: false,
    // Fill each line by measure width rather than keeping the publisher's
    // line breaks (made for paper, often 2-4 bars per line).
    breaks: 'smart',
    spacingLinear: 0.2,
    spacingNonLinear: 0.55,
    header: 'none',
    footer: 'none',
    pageMarginTop: 30,
    pageMarginBottom: 30,
    pageMarginLeft: 30,
    pageMarginRight: 30,
    svgViewBox: true,
    svgHtml5: true,
  })
  tk.loadData(musicxml)
  const pages = tk.getPageCount()

  // Measure ids in document order; written measure i is the i-th <measure>.
  const measurePage = new Map<number, { page: number; id: string }>()
  const mei = tk.getMEI({})
  const ids = [...mei.matchAll(/<measure\b[^>]*xml:id="([^"]+)"/g)].map((m) => m[1])
  ids.forEach((id, i) => {
    const page = tk.getPageWithElement(id)
    measurePage.set(i, { page, id })
  })
  return { pages, measurePage, render: (p: number) => tk.renderToSVG(p) }
}

/** Part names in a MusicXML document, in order. */
export function partNames(musicxml: string): string[] {
  const doc = new DOMParser().parseFromString(musicxml, 'application/xml')
  return [...doc.querySelectorAll('part-list > score-part')].map(
    (sp, i) => sp.querySelector('part-name')?.textContent?.trim() || `Stämma ${i + 1}`,
  )
}

const STEP: Record<string, number> = { C: 0, D: 2, E: 4, F: 5, G: 7, A: 9, B: 11 }

/** Parts written without a clef (some converted files) get one from their
 *  range: treble, treble-8 (tenor) or bass. Otherwise Verovio assumes
 *  treble and low parts sit on ledger lines. */
function addMissingClefs(doc: Document) {
  for (const part of doc.querySelectorAll('score-partwise > part')) {
    if (part.querySelector('clef')) continue
    const keys: number[] = []
    for (const p of part.querySelectorAll('note > pitch')) {
      const st = STEP[p.querySelector('step')?.textContent ?? 'C'] ?? 0
      const oct = Number(p.querySelector('octave')?.textContent ?? 4)
      keys.push((oct + 1) * 12 + st + Number(p.querySelector('alter')?.textContent ?? 0))
    }
    if (!keys.length) continue
    keys.sort((a, b) => a - b)
    const med = keys[keys.length >> 1]
    const [sign, line, oct] = med >= 64 ? ['G', '2', 0] : med >= 55 ? ['G', '2', -1] : ['F', '4', 0]
    let attrs = part.querySelector('measure > attributes')
    if (!attrs) {
      attrs = doc.createElement('attributes')
      part.querySelector('measure')?.prepend(attrs)
    }
    const clef = doc.createElement('clef')
    clef.innerHTML = `<sign>${sign}</sign><line>${line}</line>${oct ? `<clef-octave-change>${oct}</clef-octave-change>` : ''}`
    // MusicXML order: divisions, key, time, staves, ..., clef.
    attrs.appendChild(clef)
  }
}

/** VIEW-04: the MusicXML prepared for notation, without the parts at
 *  `hidden` (indices in document order). Sound is unaffected. */
export function withoutParts(musicxml: string, hidden: number[]): string {
  if (!musicxml) return musicxml
  const doc = new DOMParser().parseFromString(musicxml, 'application/xml')
  if (doc.querySelector('parsererror')) return musicxml
  const missing = [...doc.querySelectorAll('score-partwise > part')].some((p) => !p.querySelector('clef'))
  if (!hidden.length && !missing) return musicxml
  addMissingClefs(doc)
  const sps = [...doc.querySelectorAll('part-list > score-part')]
  if (hidden.length >= sps.length) return new XMLSerializer().serializeToString(doc)
  for (const i of hidden) {
    const sp = sps[i]
    if (!sp) continue
    const id = sp.getAttribute('id')
    sp.remove()
    doc.querySelectorAll(`score-partwise > part`).forEach((p) => {
      if (p.getAttribute('id') === id) p.remove()
    })
  }
  return new XMLSerializer().serializeToString(doc)
}
