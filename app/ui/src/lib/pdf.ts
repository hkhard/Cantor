// Original PDF pages (pdf.js) with the measure map for the play cursor.
import * as pdfjs from 'pdfjs-dist'
import workerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url'

pdfjs.GlobalWorkerOptions.workerSrc = workerUrl

/** VIEW-02 page modes. */
export type PageMode = 'single' | 'two' | 'half' | 'scroll'

export interface MeasureBox {
  page: number
  x: number
  y: number
  w: number
  h: number
}

/** URL of a bundled score file on the Rust side (custom URI scheme). On
 *  iOS/macOS WebKit uses `cantor://localhost/`, elsewhere
 *  `http://cantor.localhost/`. */
export function scoreFileUrl(name: string): string {
  const ua = navigator.userAgent
  const webkit = /Macintosh|iPad|iPhone/.test(ua) && !/Windows|Android/.test(ua)
  const base = webkit ? 'cantor://localhost/' : 'http://cantor.localhost/'
  return base + encodeURIComponent(name)
}

export async function openPdf(url: string): Promise<pdfjs.PDFDocumentProxy> {
  const resp = await fetch(url)
  if (!resp.ok) throw new Error(`PDF ${resp.status}`)
  const data = new Uint8Array(await resp.arrayBuffer())
  const base = new URL('/pdfjs/', location.href).href
  return pdfjs.getDocument({
    data,
    wasmUrl: base + 'wasm/',
    standardFontDataUrl: base + 'standard_fonts/',
    cMapUrl: base + 'cmaps/',
    cMapPacked: true,
    iccUrl: base + 'iccs/',
  }).promise
}

/** Render a page to fit `width` x `height` CSS pixels; returns the canvas
 *  size in CSS pixels so overlays can be placed. */
export async function renderPage(
  doc: pdfjs.PDFDocumentProxy,
  n: number,
  canvas: HTMLCanvasElement,
  width: number,
  height: number,
): Promise<{ w: number; h: number }> {
  const page = await doc.getPage(n)
  const base = page.getViewport({ scale: 1 })
  const scale = Math.min(width / base.width, height / base.height)
  const dpr = window.devicePixelRatio || 1
  const vp = page.getViewport({ scale: scale * dpr })
  canvas.width = Math.floor(vp.width)
  canvas.height = Math.floor(vp.height)
  canvas.style.width = `${Math.floor(vp.width / dpr)}px`
  canvas.style.height = `${Math.floor(vp.height / dpr)}px`
  const ctx = canvas.getContext('2d')!
  await page.render({ canvasContext: ctx, viewport: vp, canvas }).promise
  return { w: vp.width / dpr, h: vp.height / dpr }
}
