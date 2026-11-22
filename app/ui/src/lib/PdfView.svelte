<script lang="ts">
  // The original page as the singer knows it (PDF), with the playing
  // measure outlined from the OMR measure map (LIB-05).
  import { onDestroy } from 'svelte'
  import type { PDFDocumentProxy } from 'pdfjs-dist'
  import { openPdf, renderPage, type MeasureBox } from './pdf'

  interface Props {
    url: string
    boxes: MeasureBox[] | null
    /** Written measure index now playing, -1 for none. */
    measure: number
    follow: boolean
    /** Called with a written measure index when the user taps a measure. */
    onseek: (measure: number) => void
    /** A-B loop range (written measures), shaded on the page. */
    loop?: [number, number] | null
  }
  let { url, boxes, measure, follow, onseek, loop = null }: Props = $props()

  let host: HTMLDivElement | undefined = $state()
  let canvas: HTMLCanvasElement | undefined = $state()
  let doc: PDFDocumentProxy | null = $state(null)
  let pages = $state(0)
  let page = $state(1)
  let size = $state({ w: 0, h: 0 })
  let error = $state('')
  let rendering = false
  let pending: number | null = null

  export function pageInfo() {
    return { page, pages }
  }

  async function load(u: string) {
    error = ''
    try {
      doc = await openPdf(u)
      pages = doc.numPages
      page = 1
      await draw()
    } catch (e) {
      error = `Kunde inte öppna PDF: ${e}`
    }
  }

  async function draw() {
    if (!doc || !canvas || !host) return
    if (rendering) {
      pending = page
      return
    }
    rendering = true
    try {
      size = await renderPage(doc, page, canvas, host.clientWidth, host.clientHeight)
    } catch (e) {
      error = String(e)
    } finally {
      rendering = false
      if (pending !== null && pending !== page) {
        pending = null
        draw()
      } else pending = null
    }
  }

  $effect(() => {
    if (url) load(url)
  })

  // Follow the playing measure onto its page.
  $effect(() => {
    const b = boxes && measure >= 0 ? boxes[measure] : null
    if (follow && b && b.page !== page && b.page <= pages) {
      page = b.page
      draw()
    }
  })

  let resizeObs: ResizeObserver | undefined
  $effect(() => {
    if (!host) return
    resizeObs = new ResizeObserver(() => draw())
    resizeObs.observe(host)
    return () => resizeObs?.disconnect()
  })
  onDestroy(() => doc?.destroy())

  export function turn(d: number) {
    const p = Math.min(pages, Math.max(1, page + d))
    if (p !== page) {
      page = p
      draw()
    }
  }

  function tap(e: MouseEvent) {
    if (!boxes || !canvas) return
    const r = canvas.getBoundingClientRect()
    const x = (e.clientX - r.left) / r.width
    const y = (e.clientY - r.top) / r.height
    const i = boxes.findIndex(
      (b) => b.page === page && x >= b.x && x <= b.x + b.w && y >= b.y && y <= b.y + b.h,
    )
    if (i >= 0) onseek(i)
  }

  let cur = $derived(boxes && measure >= 0 ? boxes[measure] : null)
  let loopBoxes = $derived(
    boxes && loop ? boxes.slice(loop[0], loop[1] + 1).filter((b) => b.page === page) : [],
  )
</script>

<div class="pdfhost" bind:this={host}>
  {#if error}<p class="error">{error}</p>{/if}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="sheet" style="width:{size.w}px;height:{size.h}px" onclick={tap}>
    <canvas bind:this={canvas}></canvas>
    {#each loopBoxes as b}
      <div
        class="loopbox"
        style="left:{b.x * size.w}px;top:{b.y * size.h}px;width:{b.w * size.w}px;height:{b.h * size.h}px"
      ></div>
    {/each}
    {#if cur && cur.page === page}
      <div
        class="cursor"
        style="left:{cur.x * size.w}px;top:{cur.y * size.h}px;width:{cur.w * size.w}px;height:{cur.h * size.h}px"
      ></div>
    {/if}
  </div>
  {#if pages > 1}
    <button class="pageturn left" onclick={() => turn(-1)} aria-label="Föregående sida"></button>
    <button class="pageturn right" onclick={() => turn(1)} aria-label="Nästa sida"></button>
  {/if}
  <span class="pageno">{page}/{pages}</span>
  {#if !boxes}
    <p class="nomap">Takterna på sidan är inte kopplade till ljudet för det här stycket, så markören visas inte.</p>
  {/if}
</div>

<style>
  .pdfhost {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #eceae4;
  }
  .sheet {
    position: relative;
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.15);
    background: white;
  }
  canvas {
    display: block;
  }
  .cursor {
    position: absolute;
    border: 3px solid rgba(184, 86, 43, 0.75);
    background: rgba(184, 86, 43, 0.1);
    border-radius: 6px;
    pointer-events: none;
    transition:
      left 0.12s,
      top 0.12s,
      width 0.12s;
  }
  .loopbox {
    position: absolute;
    background: rgba(184, 86, 43, 0.08);
    border-top: 3px solid rgba(184, 86, 43, 0.5);
    border-bottom: 3px solid rgba(184, 86, 43, 0.5);
    pointer-events: none;
  }
  .pageturn {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 12%;
    background: transparent;
    border: none;
    border-radius: 0;
  }
  .pageturn.left {
    left: 0;
  }
  .pageturn.right {
    right: 0;
  }
  .pageno {
    position: absolute;
    bottom: 6px;
    right: 12px;
    font-size: 12px;
    color: #77716a;
  }
  .nomap {
    position: absolute;
    bottom: 6px;
    left: 12px;
    right: 80px;
    font-size: 12px;
    color: #77716a;
    margin: 0;
  }
  .error {
    position: absolute;
    top: 12px;
    color: #b91c1c;
  }
</style>
