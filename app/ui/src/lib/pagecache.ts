// PERF-05/NFR-03: rendered PDF pages, kept and drawn ahead.
//
// pdf.js draws a page on the main thread in 15 ms slices; on a 60 Hz iPad a
// slice plus the swap overran a frame at every page turn (measured on the
// iPad mini: one 34 ms frame per turn). So pages are rendered before they
// are needed (the neighbours of the page on screen, while the reader is
// still looking at it) and kept as finished bitmaps: a turn only copies a
// bitmap onto the visible canvas.
import type { PDFDocumentProxy } from 'pdfjs-dist'

export interface Rendered {
  /** The finished page at device pixels, already in the skin's colours. */
  bitmap: HTMLCanvasElement
  /** Size in CSS pixels. */
  w: number
  h: number
}

/** Pages kept per document: the visible ones and their neighbours. */
const KEEP = 8

const cache = new WeakMap<PDFDocumentProxy, Map<string, Promise<Rendered | null>>>()

function key(n: number, w: number, h: number, dpr: number, skin: string) {
  return `${n}|${Math.round(w)}|${Math.round(h)}|${dpr}|${skin}`
}

function hex(v: string): [number, number, number] | null {
  const m = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(v)
  return m ? [parseInt(m[1], 16), parseInt(m[2], 16), parseInt(m[3], 16)] : null
}

/** Map each pixel's lightness onto ink..paper (a duotone). Colours in the
 * PDF become shades of the ink: right for reading music. */
function recolour(c: HTMLCanvasElement, bg: number[], fg: number[]) {
  const ctx = c.getContext('2d')!
  const img = ctx.getImageData(0, 0, c.width, c.height)
  const d = img.data
  const lut = [new Uint8ClampedArray(256), new Uint8ClampedArray(256), new Uint8ClampedArray(256)]
  for (let l = 0; l < 256; l++) for (let k = 0; k < 3; k++) lut[k][l] = fg[k] + ((bg[k] - fg[k]) * l) / 255
  for (let i = 0; i < d.length; i += 4) {
    const l = (d[i] * 77 + d[i + 1] * 150 + d[i + 2] * 29) >> 8
    d[i] = lut[0][l]
    d[i + 1] = lut[1][l]
    d[i + 2] = lut[2][l]
  }
  ctx.putImageData(img, 0, 0)
}

async function render(doc: PDFDocumentProxy, n: number, w: number, h: number, dpr: number): Promise<Rendered | null> {
  const page = await doc.getPage(n)
  const base = page.getViewport({ scale: 1 })
  const scale = h > 0 ? Math.min(w / base.width, h / base.height) : w / base.width
  const vp = page.getViewport({ scale: scale * dpr })
  const off = document.createElement('canvas')
  off.width = Math.floor(vp.width)
  off.height = Math.floor(vp.height)
  // VIEW-06: the page follows the skin's paper and ink (black on white,
  // black on sepia, light grey on black). Rendered as usual, then every
  // pixel's lightness is mapped from ink (dark) to paper (light), so
  // vector music and scanned pages (images) are recoloured alike.
  const css = getComputedStyle(document.documentElement)
  const bg = hex(css.getPropertyValue('--paper').trim()) ?? [255, 255, 255]
  const fg = hex(css.getPropertyValue('--paper-ink').trim()) ?? [0, 0, 0]
  await page.render({ canvasContext: off.getContext('2d')!, viewport: vp, canvas: off }).promise
  if (!(bg.every((v) => v === 255) && fg.every((v) => v === 0))) recolour(off, bg, fg)
  return { bitmap: off, w: vp.width / dpr, h: vp.height / dpr }
}

/** Page `n` fitted into `w` x `h` CSS pixels (h = 0: width only), from the
 * cache or rendered now. Shared: two callers asking for the same page wait
 * for one render. */
export function renderedPage(doc: PDFDocumentProxy, n: number, w: number, h: number): Promise<Rendered | null> {
  const dpr = window.devicePixelRatio || 1
  const skin = document.documentElement.dataset.skin ?? ''
  let m = cache.get(doc)
  if (!m) cache.set(doc, (m = new Map()))
  const k = key(n, w, h, dpr, skin)
  let p = m.get(k)
  if (p) {
    // Most recently used goes last.
    m.delete(k)
    m.set(k, p)
    return p
  }
  p = render(doc, n, w, h, dpr).catch(() => null)
  m.set(k, p)
  // A failed render is not kept, so the next ask tries again.
  p.then((r) => {
    if (!r) m.delete(k)
  })
  while (m.size > KEEP) m.delete(m.keys().next().value as string)
  return p
}

/** Render pages ahead while the browser is idle (after the visible one). */
export function prefetch(doc: PDFDocumentProxy, pages: number[], w: number, h: number) {
  const idle: (cb: () => void) => void =
    'requestIdleCallback' in window
      ? (cb) => (window as unknown as { requestIdleCallback: (f: () => void, o: object) => void }).requestIdleCallback(cb, { timeout: 500 })
      : (cb) => setTimeout(cb, 120)
  let i = 0
  const next = () => {
    const n = pages[i++]
    if (n == null) return
    renderedPage(doc, n, w, h).then(() => idle(next))
  }
  idle(next)
}
