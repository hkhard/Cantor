<script lang="ts">
  // One PDF page with its overlays: play cursor, loop shading, annotations.
  // Fits inside `fitW` x `fitH` CSS pixels (fitH = 0: fit width only).
  import type { PDFDocumentProxy, RenderTask } from 'pdfjs-dist'
  import type { MeasureBox } from './pdf'
  import AnnotLayer from './AnnotLayer.svelte'
  import { t } from './i18n.svelte'
  import type { AnnDoc, AnnCommand, Tool } from './annot'

  interface Props {
    doc: PDFDocumentProxy
    n: number
    fitW: number
    fitH: number
    boxes: MeasureBox[] | null
    measure: number
    loop: [number, number] | null
    ann: AnnDoc | null
    tool: Tool | null
    inkColor: string
    layer: number
    stamp: string
    onedit: (c: AnnCommand) => void
    onseek: (m: number) => void
    /** CSS clip, for the half-page turn. */
    clip?: string
    el?: HTMLDivElement
  }
  let {
    doc,
    n,
    fitW,
    fitH,
    boxes,
    measure,
    loop,
    ann,
    tool,
    inkColor,
    layer,
    stamp,
    onedit,
    onseek,
    clip = '',
    el = $bindable(),
  }: Props = $props()

  let canvas: HTMLCanvasElement | undefined = $state()
  let size = $state({ w: 0, h: 0 })
  let task: RenderTask | null = null
  // Re-render when the skin changes (paper and ink are tokens on <html>).
  let skin = $state(document.documentElement.dataset.skin ?? '')
  $effect(() => {
    const mo = new MutationObserver(() => (skin = document.documentElement.dataset.skin ?? ''))
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ['data-skin'] })
    return () => mo.disconnect()
  })

  /** "#RRGGBB" to [r, g, b]. */
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

  async function draw(d: PDFDocumentProxy, p: number, w: number, h: number) {
    if (!canvas || w < 10) return
    task?.cancel()
    const page = await d.getPage(p)
    const base = page.getViewport({ scale: 1 })
    const scale = h > 0 ? Math.min(w / base.width, h / base.height) : w / base.width
    const dpr = window.devicePixelRatio || 1
    const vp = page.getViewport({ scale: scale * dpr })
    // Render off-screen first so a cancelled render never blanks the page.
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
    const t = page.render({ canvasContext: off.getContext('2d')!, viewport: vp, canvas: off })
    task = t
    try {
      await t.promise
    } catch {
      return // cancelled by a newer render
    }
    if (task !== t || !canvas) return
    if (!(bg.every((v) => v === 255) && fg.every((v) => v === 0))) recolour(off, bg, fg)
    canvas.width = off.width
    canvas.height = off.height
    canvas.getContext('2d')!.drawImage(off, 0, 0)
    size = { w: vp.width / dpr, h: vp.height / dpr }
    canvas.style.width = `${size.w}px`
    canvas.style.height = `${size.h}px`
  }

  $effect(() => {
    void skin
    draw(doc, n, fitW, fitH)
  })

  function tap(e: MouseEvent) {
    if (tool || !boxes || !canvas) return
    const r = canvas.getBoundingClientRect()
    const x = (e.clientX - r.left) / r.width
    const y = (e.clientY - r.top) / r.height
    const i = boxes.findIndex((b) => b.page === n && x >= b.x && x <= b.x + b.w && y >= b.y && y <= b.y + b.h)
    if (i >= 0) onseek(i)
  }

  let cur = $derived(boxes && measure >= 0 ? boxes[measure] : null)
  let loopBoxes = $derived(boxes && loop ? boxes.slice(loop[0], loop[1] + 1).filter((b) => b.page === n) : [])
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="sheet" bind:this={el} style="width:{size.w}px;height:{size.h}px;{clip ? `clip-path:${clip}` : ''}" onclick={tap}>
  <canvas bind:this={canvas} role="img" aria-label={t('a11y.page', { n })}></canvas>
  {#each loopBoxes as b}
    <div class="loopbox" style="left:{b.x * size.w}px;top:{b.y * size.h}px;width:{b.w * size.w}px;height:{b.h * size.h}px"></div>
  {/each}
  {#if cur && cur.page === n}
    <div class="cursor" style="left:{cur.x * size.w}px;top:{cur.y * size.h}px;width:{cur.w * size.w}px;height:{cur.h * size.h}px"></div>
  {/if}
  <AnnotLayer doc={ann} page={n} w={size.w} h={size.h} {tool} color={inkColor} {layer} {stamp} {onedit} {boxes} />
</div>

<style>
  .sheet {
    position: relative;
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.15);
    background: var(--paper, white);
    flex: none;
  }
  canvas {
    display: block;
  }
  .cursor {
    position: absolute;
    border: 3px solid var(--cursor-stroke);
    background: var(--cursor-fill);
    border-radius: 6px;
    pointer-events: none;
    transition:
      left 0.12s,
      top 0.12s,
      width 0.12s;
  }
  .loopbox {
    position: absolute;
    background: var(--accent-soft);
    border-top: 3px solid var(--cursor-stroke);
    border-bottom: 3px solid var(--cursor-stroke);
    pointer-events: none;
  }
</style>
