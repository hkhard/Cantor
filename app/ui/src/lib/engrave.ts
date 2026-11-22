// Verovio engraving: MusicXML -> SVG pages, and written measure -> page/element
// lookup for the play cursor.
import createVerovioModule from 'verovio/wasm'
import { VerovioToolkit } from 'verovio/esm'

let toolkitPromise: Promise<VerovioToolkit> | null = null

function toolkit(): Promise<VerovioToolkit> {
  if (!toolkitPromise) {
    toolkitPromise = createVerovioModule().then((m: unknown) => new VerovioToolkit(m))
  }
  return toolkitPromise
}

export interface Engraved {
  pages: number
  /** written measure index -> { page, id } */
  measurePage: Map<number, { page: number; id: string }>
  render(page: number): string
}

export async function engrave(musicxml: string, width: number, height: number): Promise<Engraved> {
  const tk = await toolkit()
  // Verovio units: page size in 1/10 mm at scale 100. A scale around 45
  // gives readable staves on an 11-13" iPad.
  const scale = 45
  tk.setOptions({
    pageWidth: Math.round((width * 100) / scale),
    pageHeight: Math.round((height * 100) / scale),
    scale,
    adjustPageHeight: false,
    breaks: 'auto',
    header: 'none',
    footer: 'none',
    pageMarginTop: 40,
    pageMarginBottom: 40,
    pageMarginLeft: 40,
    pageMarginRight: 40,
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
