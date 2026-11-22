<script lang="ts">
  // ANN-01..03: annotations drawn over one PDF page. Positions are page
  // fractions (from cantor-annotations), so ink scales with zoom and page size.
  // With a tool selected, Apple Pencil (or a finger) draws, erases or places
  // text and stamps; otherwise the layer is transparent to taps.
  import type { AnnDoc, AnnItem, Tool, AnnCommand } from './annot'
  import { glyph } from './annot'
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
  }
  let { doc, page, w, h, tool, color, layer, stamp, onedit }: Props = $props()

  type P = { x: number; y: number; pressure: number }
  let live: P[] = $state([])
  let erasing: [number, number][] = []
  let drawing = false

  let items = $derived(
    doc
      ? doc.items.filter(
          (a) => 'Page' in a.anchor && a.anchor.Page.page === page && doc!.layers[a.layer]?.visible !== false,
        )
      : [],
  )

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

  function up() {
    if (!drawing) return
    drawing = false
    if (tool === 'eraser') {
      if (erasing.length) onedit({ op: 'Erase', page, points: erasing, radius: 0.015 })
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

  function add(at: { x: number; y: number }, item: AnnItem): AnnCommand {
    return { op: 'Add', layer, anchor: { Page: { page, x: at.x, y: at.y } }, item }
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
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
>
  {#each items as a (a.id)}
    {@const at = (a.anchor as any).Page}
    {#if 'Ink' in a.item}
      {@const ink = a.item.Ink}
      {#if ink.tool === 'Highlighter'}
        <path
          d={path(ink.points, at.x, at.y)}
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
            x1={(at.x + p0.x) * w}
            y1={(at.y + p0.y) * h}
            x2={(at.x + q.x) * w}
            y2={(at.y + q.y) * h}
            stroke={ink.color}
            stroke-width={(ink.width * h * (p0.pressure + q.pressure)) / 2}
            stroke-linecap="round"
          />
        {/each}
      {/if}
    {:else if 'Text' in a.item}
      <text x={at.x * w} y={at.y * h} font-size={a.item.Text.size * h} fill={a.item.Text.color}>{a.item.Text.text}</text>
    {:else if 'Stamp' in a.item}
      <text
        x={at.x * w}
        y={at.y * h}
        font-size={a.item.Stamp.scale * h}
        font-style="italic"
        font-weight="bold"
        fill="#1d1d1f"
        text-anchor="middle">{glyph(a.item.Stamp.kind)}</text
      >
    {:else if 'Raster' in a.item && a.item.Raster.page === page}
      <image href="data:image/png;base64,{a.item.Raster.png_base64 ?? ''}" x="0" y="0" width={w} height={h} />
    {/if}
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
