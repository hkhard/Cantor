<script lang="ts">
  // ANN-01..04: annotations drawn over one page (a PDF page or an engraved
  // notation page). Positions are page fractions, so ink scales with zoom.
  // When the page's measure boxes are known (PDF measure map, or the bars of
  // the engraved notation), new marks are anchored to the bar under the pen
  // and stored in bar units: they stay on their bar when the notation is laid
  // out again, and show on both the original page and the notation.
  // With a tool selected, Apple Pencil (or a finger) draws, erases or places
  // text and stamps; otherwise the layer is transparent to taps.
  import type { AnnDoc, AnnItem, Tool, AnnCommand } from './annot'
  import { glyph, placeOnPage, type Box } from './annot'
  import { ask } from './prompts.svelte'
  import { t } from './i18n.svelte'

  interface Props {
    doc: AnnDoc | null
    page: number
    w: number
    h: number
    tool: Tool | null
    color: string
    layer: number
    stamp: string
    onedit: (c: AnnCommand) => void
    /** Measure boxes by written measure index (page fractions; null when unknown). */
    boxes?: (Box | null)[] | null
    /** Notation pages: only bar-anchored items (page-anchored ones belong to the PDF). */
    measureOnly?: boolean
  }
  let { doc, page, w, h, tool, color, layer, stamp, onedit, boxes = null, measureOnly = false }: Props = $props()

  type P = { x: number; y: number; pressure: number }
  let live: P[] = $state([])
  let erasing: [number, number][] = []
  let drawing = false

  /** Visible items placed on this page, in page units. */
  let items = $derived.by(() => {
    if (!doc) return []
    const out: { id: number; x: number; y: number; item: AnnItem }[] = []
    for (const a of doc.items) {
      if (doc.layers[a.layer]?.visible === false) continue
      if (measureOnly && !('Measure' in a.anchor)) continue
      const p = placeOnPage(a, boxes)
      if (p && p.page === page) out.push({ id: a.id, x: p.x, y: p.y, item: p.item })
    }
    return out
  })

  function pt(e: PointerEvent, el: Element): P {
    const r = el.getBoundingClientRect()
    // Finger touches report pressure 0 or 0.5: treat both as a normal stroke.
    const pr = e.pointerType === 'pen' ? Math.max(0.15, e.pressure * 1.6) : 1
    return { x: (e.clientX - r.left) / r.width, y: (e.clientY - r.top) / r.height, pressure: Math.min(pr, 1.6) }
  }

  function down(e: PointerEvent) {
    if (!tool) return
    e.preventDefault()
    e.stopPropagation()
    const el = e.currentTarget as Element
    const p = pt(e, el)
    if (tool === 'text') {
      const at = { x: p.x, y: p.y }
      const c = color
      ask(t('ann.textPrompt')).then((text) => {
        if (text) onedit(add(at, { Text: { text, size: 0.022, color: c } }))
      })
      return
    }
    if (tool === 'stamp') {
      onedit(add({ x: p.x, y: p.y }, { Stamp: { kind: stampKind(stamp), scale: 0.03 } }))
      return
    }
    ;(el as HTMLElement).setPointerCapture(e.pointerId)
    drawing = true
    if (tool === 'eraser') erasing = [[p.x, p.y]]
    else live = [p]
  }

  function move(e: PointerEvent) {
    if (!drawing) return
    e.preventDefault()
    const el = e.currentTarget as Element
    // Pencil delivers 240 Hz samples as coalesced events.
    const evs = e.getCoalescedEvents?.() ?? [e]
    for (const ev of evs.length ? evs : [e]) {
      const p = pt(ev, el)
      if (tool === 'eraser') erasing.push([p.x, p.y])
      else live.push(p)
    }
    if (tool !== 'eraser') live = live
  }

  /** Ink items on this page touched by the eraser path. */
  function erased(): number[] {
    const r = 0.015
    const hit: number[] = []
    for (const it of items) {
      if (!('Ink' in it.item)) continue
      const near = it.item.Ink.points.some((q) =>
        erasing.some(([ex, ey]) => {
          const dx = (it.x + q.x - ex) * (w / h)
          const dy = it.y + q.y - ey
          return dx * dx + dy * dy <= r * r
        }),
      )
      if (near) hit.push(it.id)
    }
    return hit
  }

  function up() {
    if (!drawing) return
    drawing = false
    if (tool === 'eraser') {
      const ids = erased()
      if (ids.length) onedit({ op: 'RemoveMany', ids })
      erasing = []
      return
    }
    const pts = live
    live = []
    if (pts.length < 2) return
    const o = pts[0]
    const hl = tool === 'highlighter'
    onedit(
      add(
        { x: o.x, y: o.y },
        {
          Ink: {
            points: pts.map((p) => ({ x: p.x - o.x, y: p.y - o.y, pressure: hl ? 1 : p.pressure })),
            color: hl ? '#f5d90a' : color,
            width: hl ? 0.012 : 0.0022,
            tool: hl ? 'Highlighter' : 'Pen',
          },
        },
      ),
    )
  }

  /** The bar under a point on this page: inside its box, else the nearest
   *  box on the page within a short distance. */
  function barAt(x: number, y: number): number {
    if (!boxes) return -1
    let best = -1
    let bestD = 0.0025 // (5% of the page)^2
    boxes.forEach((b, i) => {
      if (!b || b.page !== page) return
      const dx = Math.max(b.x - x, 0, x - (b.x + b.w))
      const dy = Math.max(b.y - y, 0, y - (b.y + b.h))
      const d = dx * dx + dy * dy
      if (d < bestD) {
        bestD = d
        best = i
      }
    })
    return best
  }

  /** Anchor a new item (given in page units) to its bar when known. */
  function add(at: { x: number; y: number }, item: AnnItem): AnnCommand {
    const m = barAt(at.x, at.y)
    const b = m >= 0 ? boxes![m] : null
    if (!b || b.w <= 0 || b.h <= 0) {
      // Notation pages have no fixed page: off a bar, use the nearest bar anyway.
      if (measureOnly) {
        const n = nearestAny(at.x, at.y)
        if (n >= 0) return anchorTo(n, at, item)
      }
      return { op: 'Add', layer, anchor: { Page: { page, x: at.x, y: at.y } }, item }
    }
    return anchorTo(m, at, item)
  }

  function nearestAny(x: number, y: number): number {
    let best = -1
    let bestD = Infinity
    boxes?.forEach((b, i) => {
      if (!b || b.page !== page) return
      const d = (b.x + b.w / 2 - x) ** 2 + (b.y + b.h / 2 - y) ** 2
      if (d < bestD) {
        bestD = d
        best = i
      }
    })
    return best
  }

  function anchorTo(m: number, at: { x: number; y: number }, item: AnnItem): AnnCommand {
    const b = boxes![m]!
    const sx = 1 / b.w
    const sy = 1 / b.h
    let local: AnnItem = item
    if ('Ink' in item)
      local = {
        Ink: {
          ...item.Ink,
          points: item.Ink.points.map((p) => ({ x: p.x * sx, y: p.y * sy, pressure: p.pressure })),
          width: item.Ink.width * sy,
        },
      }
    else if ('Text' in item) local = { Text: { ...item.Text, size: item.Text.size * sy } }
    else if ('Stamp' in item) local = { Stamp: { ...item.Stamp, scale: item.Stamp.scale * sy } }
    return {
      op: 'Add',
      layer,
      anchor: { Measure: { measure: m, staff: null, dx: (at.x - b.x) * sx, dy: (at.y - b.y) * sy } },
      item: local,
    }
  }

  function stampKind(s: string): unknown {
    if (/^[1-5]$/.test(s)) return { Fingering: Number(s) }
    const unit = ['Breath', 'Fermata', 'Pp', 'P', 'Mp', 'Mf', 'F', 'Ff', 'Crescendo', 'Diminuendo', 'Accent', 'Tenuto', 'Staccato']
    return unit.includes(s) ? s : { Custom: s }
  }

  function path(pts: { x: number; y: number }[], ox: number, oy: number) {
    return pts.map((p, i) => `${i ? 'L' : 'M'}${(ox + p.x) * w} ${(oy + p.y) * h}`).join(' ')
  }

</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<svg
  class="annot"
  class:active={!!tool}
  width={w}
  height={h}
  viewBox="0 0 {w} {h}"
  aria-hidden={!tool}
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
>
  {#each items as a (a.id)}
    <g data-ann={a.id}>
    {#if 'Ink' in a.item}
      {@const ink = a.item.Ink}
      {#if ink.tool === 'Highlighter'}
        <path
          d={path(ink.points, a.x, a.y)}
          fill="none"
          stroke={ink.color}
          stroke-opacity="0.4"
          stroke-width={ink.width * h}
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      {:else}
        {#each ink.points.slice(1) as q, i}
          {@const p0 = ink.points[i]}
          <line
            x1={(a.x + p0.x) * w}
            y1={(a.y + p0.y) * h}
            x2={(a.x + q.x) * w}
            y2={(a.y + q.y) * h}
            stroke={ink.color}
            stroke-width={(ink.width * h * (p0.pressure + q.pressure)) / 2}
            stroke-linecap="round"
          />
        {/each}
      {/if}
    {:else if 'Text' in a.item}
      <text x={a.x * w} y={a.y * h} font-size={a.item.Text.size * h} fill={a.item.Text.color}>{a.item.Text.text}</text>
    {:else if 'Stamp' in a.item}
      <text
        x={a.x * w}
        y={a.y * h}
        font-size={a.item.Stamp.scale * h}
        font-style="italic"
        font-weight="bold"
        fill="#1d1d1f"
        text-anchor="middle">{glyph(a.item.Stamp.kind)}</text
      >
    {:else if 'Raster' in a.item && a.item.Raster.page === page}
      <image href="data:image/png;base64,{a.item.Raster.png_base64 ?? ''}" x="0" y="0" width={w} height={h} />
    {/if}
    </g>
  {/each}
  {#if live.length > 1}
    <path
      d={path(live, 0, 0)}
      fill="none"
      stroke={tool === 'highlighter' ? '#f5d90a' : color}
      stroke-opacity={tool === 'highlighter' ? 0.4 : 1}
      stroke-width={tool === 'highlighter' ? 0.012 * h : 0.0022 * h}
      stroke-linecap="round"
      stroke-linejoin="round"
    />
  {/if}
</svg>

<style>
  .annot {
    position: absolute;
    inset: 0;
    pointer-events: none;
    touch-action: none;
  }
  .annot.active {
    pointer-events: auto;
    cursor: crosshair;
  }
</style>
