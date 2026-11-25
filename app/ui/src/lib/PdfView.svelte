<script lang="ts">
  // The original page as the singer knows it (PDF), with the playing
  // measure outlined from the OMR measure map (LIB-05), annotations
  // (ANN-01..03) and page modes (VIEW-02): single page, two pages side by
  // side, half-page turns (the bottom of this page stays while the top of
  // the next one appears, as on a music stand), and continuous scroll.
  import { onDestroy, tick, untrack } from 'svelte'
  import type { PDFDocumentProxy } from 'pdfjs-dist'
  import { closePdf, openPdf, type MeasureBox, type PageMode } from './pdf'
  import PdfPage from './PdfPage.svelte'
  import { prefetch, readyPage } from './pagecache'
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
    /** PERF-06: when set, the side zones call this (bar by bar) instead of
     * turning the page. */
    onside?: ((d: number) => void) | null
    /** PERF-07: the shown page and page count, whenever they change. */
    onpage?: (page: number, pages: number) => void
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
    onside = null,
    onpage = () => {},
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

  $effect(() => {
    onpage(page, pages)
  })

  export function pageInfo() {
    return { page, pages }
  }

  async function load(u: string) {
    error = ''
    try {
      const d = await openPdf(u)
      closePdf(doc)
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
  onDestroy(() => closePdf(doc))

  // A half-turned page only exists in half-page mode.
  $effect(() => {
    if (mode !== 'half') half = false
  })

  // Two pages side by side only when there is room (landscape).
  let twoUp = $derived(mode === 'two' && box.w > box.h * 1.1)
  let step = $derived(twoUp ? 2 : 1)
  // VIEW-07: in landscape a single page is zoomed to the pane's width (a
  // system as wide as the screen) and scrolls vertically. If the tallest
  // system would not fit the pane at that zoom, zoom out just enough.
  // VIEW-08: on a phone in portrait too (a whole page is unreadably small).
  let fitWidth = $derived((mode === 'single' || mode === 'half') && (box.w > box.h * 1.1 || Math.min(window.innerWidth, window.innerHeight) < 600))
  let fitEl: HTMLDivElement | undefined = $state()
  let fitH = $derived.by(() => {
    if (!fitWidth || !boxes?.length) return 0
    const tallest = Math.max(...boxes.map((b) => b.h))
    // Page height allowed so the tallest system still fits, with a margin.
    return tallest > 0 ? (box.h * 0.94) / tallest : 0
  })
  // NFR-03: draw the pages a turn will show next while the reader is still
  // on this one (one ahead and one behind; two ahead in a spread), at the
  // size the current mode shows them, so a turn only copies a bitmap.
  $effect(() => {
    if (!doc || box.w <= 0 || mode === 'scroll') return
    const d = doc
    const [w, h] = fitWidth ? [box.w - 8, fitH] : twoUp ? [box.w / 2 - 6, box.h] : [box.w, box.h]
    const near = twoUp ? [page + 2, page + 3, page - 2, page - 1] : [page + 1, page - 1, page + 2]
    const todo = near.filter((p) => p >= 1 && p <= pages)
    untrack(() => prefetch(d, todo, w, h))
  })
  /** PERF-05: the page(s) a whole-page turn in direction `d` will show,
   * already drawn, with where they will sit on screen (viewport pixels).
   * The page curl shows them under the lifted sheet, as in a book; null
   * when there is nothing drawn yet (the curl then shows plain paper). */
  export function peek(d: number): { bitmap: HTMLCanvasElement; x: number; y: number; w: number; h: number; page: number }[] | null {
    if (!doc || !host || mode === 'scroll') return null
    let p = page + d * step
    if (p < 1 || p > pages) return null
    if (twoUp && p % 2 === 0) p -= 1
    const hr = host.getBoundingClientRect()
    if (fitWidth) {
      const r = readyPage(doc, p, box.w - 8, fitH)
      if (!r || !fitEl) return null
      // show() scrolls the new page to its first system.
      const b = boxes?.find((x) => x.page === p)
      const scroll = b ? Math.max(0, fitEl.offsetTop + b.y * r.h - 8) : 0
      return [{ page: p, bitmap: r.bitmap, x: hr.left + (hr.width - r.w) / 2, y: hr.top + fitEl.offsetTop - scroll, w: r.w, h: r.h }]
    }
    if (twoUp) {
      const a = readyPage(doc, p, box.w / 2 - 6, box.h)
      const c = p + 1 <= pages ? readyPage(doc, p + 1, box.w / 2 - 6, box.h) : null
      if (!a || (p + 1 <= pages && !c)) return null
      const total = a.w + (c ? c.w + 12 : 0)
      const x0 = hr.left + (hr.width - total) / 2
      const out = [{ page: p, bitmap: a.bitmap, x: x0, y: hr.top + (hr.height - a.h) / 2, w: a.w, h: a.h }]
      if (c) out.push({ page: p + 1, bitmap: c.bitmap, x: x0 + a.w + 12, y: hr.top + (hr.height - c.h) / 2, w: c.w, h: c.h })
      return out
    }
    const r = readyPage(doc, p, box.w, box.h)
    return r ? [{ page: p, bitmap: r.bitmap, x: hr.left + (hr.width - r.w) / 2, y: hr.top + (hr.height - r.h) / 2, w: r.w, h: r.h }] : null
  }

  /** Top of the first system on page `p`, in host scroll pixels. */
  function firstSystemTop(p: number): number {
    const b = boxes?.find((x) => x.page === p)
    return b && fitEl ? Math.max(0, fitEl.offsetTop + b.y * fitEl.clientHeight - 8) : 0
  }
  // A width-fitted page opens at its first system (the title block is
  // above it), also on the first page and after a rotation.
  let fittedFor = ''
  $effect(() => {
    if (!fitWidth || !fitEl || !host) return
    const key = `${url}|${page}|${Math.round(box.w)}`
    if (key === fittedFor) return
    fittedFor = key
    const el = fitEl
    const h = host
    // Wait for the canvas to get its size.
    const go = () => (el.clientHeight > 40 ? h.scrollTo({ top: firstSystemTop(page) }) : requestAnimationFrame(go))
    requestAnimationFrame(go)
  })

  function show(p: number) {
    p = Math.min(pages, Math.max(1, p))
    if (twoUp && p % 2 === 0) p -= 1
    page = p
    half = false
    if (mode === 'scroll') {
      tick().then(() => pageEls[p - 1]?.scrollIntoView({ behavior: 'smooth', block: 'start' }))
    }
    // Width-fitted page: start at its first system (the title is above).
    if (fitWidth) tick().then(() => host?.scrollTo({ top: firstSystemTop(p) }))
  }

  /** PERF-05: a whole page, whatever the mode (swipe). */
  export function turnPage(d: number) {
    if (mode === 'scroll') return turn(d)
    show(page + d * step)
  }

  /** PERF-04: show a page (turn marker on the backing track). */
  export function goto(p: number) {
    show(p)
  }

  export function turn(d: number) {
    if (fitWidth && host) {
      // Down the page first, then on to the next one.
      const more = host.scrollTop + host.clientHeight < host.scrollHeight - 4
      if (d > 0 && more) host.scrollBy({ top: host.clientHeight * 0.85, behavior: 'smooth' })
      else if (d < 0 && host.scrollTop > firstSystemTop(page) + 4)
        host.scrollBy({ top: -host.clientHeight * 0.85, behavior: 'smooth' })
      else show(page + d)
      return
    }
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

  // Follow the playing measure onto its page. Only when the playing bar
  // changes: reading `page` here would snap manual page turns straight back.
  let followed = -1
  $effect(() => {
    const m = measure
    if (m === followed) return
    followed = m
    untrack(() => followTo(m))
  })
  function followTo(measure: number) {
    const b = boxes && measure >= 0 ? boxes[measure] : null
    if (!follow || !b || b.page > pages) return
    if (fitWidth) {
      const reveal = () => {
        if (!host || !fitEl) return
        const top = fitEl.offsetTop + b.y * fitEl.clientHeight
        const bottom = fitEl.offsetTop + (b.y + b.h) * fitEl.clientHeight
        if (top < host.scrollTop || bottom > host.scrollTop + host.clientHeight)
          host.scrollTo({ top: Math.max(0, top - 8), behavior: 'smooth' })
      }
      if (b.page !== page) {
        page = b.page
        tick().then(reveal)
      } else reveal()
      return
    }
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
  }

  let common = $derived({ boxes, measure, loop, ann, tool, inkColor, layer, stamp, onedit, onseek })
</script>

<div class="pdfwrap">
<div class="pdfhost" class:scroll={mode === 'scroll' || fitWidth} bind:this={host}>
  {#if error}<p class="error">{error}</p>{/if}
  {#if doc && box.w > 0}
    {#if fitWidth}
      <div class="column">
        <PdfPage {doc} n={page} fitW={box.w - 8} {fitH} {...common} bind:el={fitEl} />
      </div>
    {:else if mode === 'scroll'}
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
</div>
  {#if !tool && onside}
    <button class="pageturn left" onclick={() => onside?.(-1)} aria-label={t('read.prevBar')}></button>
    <button class="pageturn right" onclick={() => onside?.(1)} aria-label={t('read.nextBar')}></button>
  {:else if pages > 1 && !tool && mode !== 'scroll'}
    <button class="pageturn left" onclick={() => turn(-1)} aria-label={t('read.prev')}></button>
    <button class="pageturn right" onclick={() => turn(1)} aria-label={t('read.next')}></button>
  {/if}
  {#if mode !== 'scroll'}
    <span class="pageno">{twoUp && page + 1 <= pages ? `${page}–${page + 1}` : page}{half ? '½' : ''}/{pages}</span>
  {/if}
  {#if !boxes && Math.min(window.innerWidth, window.innerHeight) >= 600}
    <!-- Not on a phone: it would sit on the music (VIEW-08). -->
    <p class="nomap">{t('pdf.noMap')}</p>
  {/if}
</div>

<style>
  .pdfwrap {
    position: absolute;
    inset: 0;
  }
  .pdfhost {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    /* Desk around the page: a step away from the paper, never a glare
       edge around a black night page. */
    background: color-mix(in srgb, var(--paper, #fff) 88%, var(--paper-ink, #000));
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
    border-top: 2px dashed var(--cursor-stroke);
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
    font-size: 0.75rem;
    /* Text on the desk: mostly ink, so >= 4.5:1 in every skin. */
    color: color-mix(in srgb, var(--paper-ink, #000) 75%, var(--paper, #fff));
  }
  .nomap {
    position: absolute;
    bottom: 6px;
    left: 12px;
    right: 80px;
    font-size: 0.75rem;
    /* Text on the desk: mostly ink, so >= 4.5:1 in every skin. */
    color: color-mix(in srgb, var(--paper-ink, #000) 75%, var(--paper, #fff));
    margin: 0;
    pointer-events: none;
  }
  .error {
    position: absolute;
    top: 12px;
    color: var(--danger, #b91c1c);
  }
</style>
