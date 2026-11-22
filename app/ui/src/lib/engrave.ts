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
