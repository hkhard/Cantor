<script lang="ts">
  // One PDF page with its overlays: play cursor, loop shading, annotations.
  // Fits inside `fitW` x `fitH` CSS pixels (fitH = 0: fit width only).
  import type { PDFDocumentProxy } from 'pdfjs-dist'
  import type { MeasureBox } from './pdf'
  import AnnotLayer from './AnnotLayer.svelte'
  import { t } from './i18n.svelte'
  import type { AnnDoc, AnnCommand, Tool } from './annot'
  import { renderedPage } from './pagecache'

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
  /** The latest draw request; an older render that finishes later is dropped. */
  let want = 0
  // Re-render when the skin changes (paper and ink are tokens on <html>).
  let skin = $state(document.documentElement.dataset.skin ?? '')
  $effect(() => {
    const mo = new MutationObserver(() => (skin = document.documentElement.dataset.skin ?? ''))
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ['data-skin'] })
    return () => mo.disconnect()
  })

  async function draw(d: PDFDocumentProxy, p: number, w: number, h: number) {
    if (!canvas || w < 10) return
    const me = ++want
    // From the page cache: drawn ahead for the neighbours of the page on
    // screen, so a turn usually only copies a finished bitmap (NFR-03).
    const r = await renderedPage(d, p, w, h)
    if (me !== want || !r || !canvas) return // a newer page was asked for
    if (canvas.width !== r.bitmap.width || canvas.height !== r.bitmap.height) {
      canvas.width = r.bitmap.width
      canvas.height = r.bitmap.height
    }
    canvas.getContext('2d')!.drawImage(r.bitmap, 0, 0)
    size = { w: r.w, h: r.h }
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
