<script lang="ts">
  // The original page as the singer knows it (PDF), with the playing
  // measure outlined from the OMR measure map (LIB-05), annotations
  // (ANN-01..03) and page modes (VIEW-02): single page, two pages side by
  // side, half-page turns (the bottom of this page stays while the top of
  // the next one appears, as on a music stand), and continuous scroll.
  import { onDestroy, tick } from 'svelte'
  import type { PDFDocumentProxy } from 'pdfjs-dist'
  import { openPdf, type MeasureBox, type PageMode } from './pdf'
  import PdfPage from './PdfPage.svelte'
  import type { AnnDoc, AnnCommand, Tool } from './annot'
  import { t } from './i18n.svelte'

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
    mode?: PageMode
    ann?: AnnDoc | null
    tool?: Tool | null
    inkColor?: string
    layer?: number
    stamp?: string
    onedit?: (c: AnnCommand) => void
  }
  let {
    url,
    boxes,
    measure,
    follow,
    onseek,
    loop = null,
    mode = 'single',
    ann = null,
    tool = null,
    inkColor = '#1d4ed8',
    layer = 0,
    stamp = 'Breath',
    onedit = () => {},
  }: Props = $props()

  let host: HTMLDivElement | undefined = $state()
  let doc: PDFDocumentProxy | null = $state(null)
  let pages = $state(0)
  let page = $state(1)
  /** Half-page mode: the top half already shows the next page. */
  let half = $state(false)
  let box = $state({ w: 0, h: 0 })
  let error = $state('')
  let pageEls: (HTMLDivElement | undefined)[] = $state([])

  export function pageInfo() {
    return { page, pages }
  }

  async function load(u: string) {
    error = ''
    try {
      const d = await openPdf(u)
      doc?.destroy()
      doc = d
      pages = d.numPages
      page = 1
      half = false
    } catch (e) {
      error = t('pdf.openFailed', { e: String(e) })
    }
  }

  $effect(() => {
    if (url) load(url)
  })

  $effect(() => {
    if (!host) return
    const ro = new ResizeObserver(() => {
      if (host) box = { w: host.clientWidth, h: host.clientHeight }
    })
    ro.observe(host)
    return () => ro.disconnect()
  })
  onDestroy(() => doc?.destroy())

  // A half-turned page only exists in half-page mode.
  $effect(() => {
    if (mode !== 'half') half = false
  })

  // Two pages side by side only when there is room (landscape).
  let twoUp = $derived(mode === 'two' && box.w > box.h * 1.1)
  let step = $derived(twoUp ? 2 : 1)

  function show(p: number) {
    p = Math.min(pages, Math.max(1, p))
    if (twoUp && p % 2 === 0) p -= 1
    page = p
    half = false
    if (mode === 'scroll') {
      tick().then(() => pageEls[p - 1]?.scrollIntoView({ behavior: 'smooth', block: 'start' }))
    }
  }

  export function turn(d: number) {
    if (mode === 'half') {
      if (d > 0) {
        if (!half && page < pages) half = true
        else if (half) show(page + 1)
      } else if (half) half = false
      else if (page > 1) {
        page -= 1
        half = true
      }
      return
    }
    if (mode === 'scroll' && host) {
      host.scrollBy({ top: d * host.clientHeight * 0.85, behavior: 'smooth' })
      return
    }
    show(page + d * step)
  }

  // Follow the playing measure onto its page.
  $effect(() => {
    const b = boxes && measure >= 0 ? boxes[measure] : null
    if (!follow || !b || b.page > pages) return
    if (mode === 'scroll') {
      const el = pageEls[b.page - 1]
      if (el && host) {
        const top = el.offsetTop + b.y * el.clientHeight
        const bottom = el.offsetTop + (b.y + b.h) * el.clientHeight
        if (top < host.scrollTop || bottom > host.scrollTop + host.clientHeight)
          host.scrollTo({ top: top - host.clientHeight * 0.2, behavior: 'smooth' })
      }
      return
    }
    if (mode === 'half') {
      // On the lower half of a page: show it. Top of the next page while the
      // previous bottom is still in view: the half-turned state.
      if (b.page === page + 1 && b.y + b.h <= 0.55) half = true
      else if (b.page !== page) show(b.page)
      return
    }
    const visible = twoUp ? b.page === page || b.page === page + 1 : b.page === page
    if (!visible) show(b.page)
  })

  let common = $derived({ boxes, measure, loop, ann, tool, inkColor, layer, stamp, onedit, onseek })
</script>

<div class="pdfhost" class:scroll={mode === 'scroll'} bind:this={host}>
  {#if error}<p class="error">{error}</p>{/if}
  {#if doc && box.w > 0}
    {#if mode === 'scroll'}
      <div class="column">
        {#each Array.from({ length: pages }, (_, i) => i + 1) as n (n)}
          <PdfPage {doc} {n} fitW={Math.min(box.w - 16, 1400)} fitH={0} {...common} bind:el={pageEls[n - 1]} />
        {/each}
      </div>
    {:else if twoUp}
      <div class="spread">
        <PdfPage {doc} n={page} fitW={box.w / 2 - 6} fitH={box.h} {...common} />
        {#if page + 1 <= pages}
          <PdfPage {doc} n={page + 1} fitW={box.w / 2 - 6} fitH={box.h} {...common} />
        {/if}
      </div>
    {:else}
      <div class="stack">
        <PdfPage {doc} n={page} fitW={box.w} fitH={box.h} {...common} />
        {#if mode === 'half' && half && page + 1 <= pages}
          <div class="halfover">
            <PdfPage {doc} n={page + 1} fitW={box.w} fitH={box.h} {...common} clip="inset(0 0 50% 0)" />
            <div class="halfline"></div>
          </div>
        {/if}
      </div>
    {/if}
  {/if}
  {#if pages > 1 && !tool && mode !== 'scroll'}
    <button class="pageturn left" onclick={() => turn(-1)} aria-label={t('read.prev')}></button>
    <button class="pageturn right" onclick={() => turn(1)} aria-label={t('read.next')}></button>
  {/if}
  {#if mode !== 'scroll'}
    <span class="pageno">{twoUp && page + 1 <= pages ? `${page}–${page + 1}` : page}{half ? '½' : ''}/{pages}</span>
  {/if}
  {#if !boxes}
    <p class="nomap">{t('pdf.noMap')}</p>
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
  .pdfhost.scroll {
    display: block;
    overflow-y: auto;
    -webkit-overflow-scrolling: touch;
  }
  .column {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    padding: 8px 0 40px;
  }
  .spread {
    display: flex;
    gap: 12px;
    align-items: center;
  }
  .stack {
    position: relative;
  }
  .halfover {
    position: absolute;
    left: 0;
    top: 0;
  }
  .halfline {
    position: absolute;
    left: 0;
    right: 0;
    top: 50%;
    border-top: 2px dashed rgba(184, 86, 43, 0.6);
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
    pointer-events: none;
  }
  .error {
    position: absolute;
    top: 12px;
    color: #b91c1c;
  }
</style>
