// PERF-05: turn pages like a book. Drag sideways and the page's leading
// edge lifts and folds back under your finger: the lifted part shows the
// back of the sheet, with a shadow along the fold. Let go past a quarter of
// the width (or flick) and the page completes the turn; otherwise it falls
// back. Transforms only (60 fps); with reduced motion the page just turns.
export interface SwipeOpts {
  enabled: boolean
  /** +1 = next (drag to the left), -1 = previous. */
  onswipe: (d: number) => void | Promise<void>
  /** Is there a page that way? At the first or last page the drag only
   * gives a little resistance (no fold) and nothing turns. */
  canturn?: (d: number) => boolean
}

const reduced = () => typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches

/** The fold drawn over the pane: `cover` hides the lifted part of the page,
 * `flap` is the back of the sheet folded over, `shade` darkens the page
 * under the flap. All positioned with transforms only. */
function makeFold(node: HTMLElement) {
  const el = document.createElement('div')
  el.className = 'pagefold'
  el.setAttribute('aria-hidden', 'true')
  el.innerHTML = '<div class="pf-cover"></div><div class="pf-shade"></div><div class="pf-flap"></div>'
  node.appendChild(el)
  const [cover, shade, flap] = Array.from(el.children) as HTMLElement[]
  return { el, cover, shade, flap }
}

/** Fold geometry for a finger at `x` (0..w) turning in direction `d`. */
function place(f: ReturnType<typeof makeFold>, w: number, x: number, d: number) {
  // Forward: the right edge follows the finger. The fold sits halfway
  // between the edge and the finger; the flap spans finger..fold.
  const edge = d > 0 ? w : 0
  // Forward the finger can travel past the left edge (the whole sheet
  // lifted at -w); backward past the right edge (2w).
  const fx = d > 0 ? Math.max(Math.min(x, w), -w) : Math.max(Math.min(x, 2 * w), 0)
  const fold = (edge + fx) / 2
  const lifted = Math.abs(edge - fold) // width of cover and flap
  const k = Math.max(lifted / w, 0.0001)
  if (d > 0) {
    f.cover.style.transformOrigin = 'right center'
    f.cover.style.transform = `scaleX(${k})`
    f.flap.style.transformOrigin = 'left center'
    f.flap.style.transform = `translateX(${fx}px) scaleX(${k})`
    f.shade.style.transform = `translateX(${fx - w * 0.06}px)`
  } else {
    f.cover.style.transformOrigin = 'left center'
    f.cover.style.transform = `scaleX(${k})`
    f.flap.style.transformOrigin = 'right center'
    f.flap.style.transform = `translateX(${fx - w}px) scaleX(${k})`
    f.shade.style.transform = `translateX(${fx}px)`
  }
  f.shade.style.opacity = String(Math.min(1, lifted / (w * 0.15)) * (1 - lifted / w))
  f.el.dataset.dir = d > 0 ? 'next' : 'prev'
}

function animate(from: number, to: number, ms: number, step: (x: number) => void): Promise<void> {
  return new Promise((done) => {
    const t0 = performance.now()
    const tick = (now: number) => {
      const p = Math.min(1, (now - t0) / ms)
      const e = 1 - (1 - p) ** 3 // ease-out
      step(from + (to - from) * e)
      if (p < 1) requestAnimationFrame(tick)
      else done()
    }
    requestAnimationFrame(tick)
  })
}

export function swipe(node: HTMLElement, opts: SwipeOpts) {
  let o = opts
  let start: { x: number; y: number; t: number; id: number } | null = null
  let drag: { d: number; f: ReturnType<typeof makeFold> | null; w: number; left: number; lastX: number; lastT: number; v: number } | null = null
  let busy = false
  // Vertical scrolling stays native; horizontal drags come to us.
  node.style.touchAction = 'pan-y pinch-zoom'

  const down = (e: PointerEvent) => {
    if (!o.enabled || !e.isPrimary || busy) return
    start = { x: e.clientX, y: e.clientY, t: performance.now(), id: e.pointerId }
  }
  const move = (e: PointerEvent) => {
    if (!start || e.pointerId !== start.id) return
    const dx = e.clientX - start.x
    const dy = e.clientY - start.y
    if (!drag) {
      // Start folding once the drag is clearly sideways.
      if (Math.abs(dx) < 12 || Math.abs(dx) < Math.abs(dy) * 1.5) {
        if (Math.abs(dy) > 24) start = null // a scroll, not a turn
        return
      }
      const r = node.getBoundingClientRect()
      const d = dx < 0 ? 1 : -1
      const can = o.canturn ? o.canturn(d) : true
      drag = { d, f: can ? makeFold(node) : null, w: r.width, left: r.left, lastX: e.clientX, lastT: e.timeStamp, v: 0 }
      node.setPointerCapture?.(e.pointerId)
    }
    if (!drag.f) {
      // Edge of the score: a short rubber band, then it springs back.
      const pull = Math.sign(dx) * Math.min(28, Math.abs(dx) / 6)
      if (!reduced()) node.style.transform = `translateX(${pull}px)`
      return
    }
    const now = e.timeStamp
    drag.v = (e.clientX - drag.lastX) / Math.max(1, now - drag.lastT)
    drag.lastX = e.clientX
    drag.lastT = now
    // The page edge starts at the side it is pulled from.
    const x = drag.d > 0 ? drag.w + dx : dx
    if (!reduced()) place(drag.f, drag.w, x, drag.d)
  }
  const up = async (e: PointerEvent) => {
    const s = start
    const g = drag
    start = null
    drag = null
    if (!s || e.pointerId !== s.id || !g) return
    if (!g.f) {
      // Nothing that way: spring back, and the release is not a tap.
      node.addEventListener('click', (c) => (c.stopPropagation(), c.preventDefault()), { capture: true, once: true })
      if (!reduced()) {
        node.style.transition = 'transform 0.18s ease-out'
        node.style.transform = ''
        setTimeout(() => (node.style.transition = ''), 200)
      } else node.style.transform = ''
      return
    }
    // The release must not also count as a tap on a bar.
    const eat = (c: Event) => {
      c.stopPropagation()
      c.preventDefault()
    }
    node.addEventListener('click', eat, { capture: true, once: true })
    setTimeout(() => node.removeEventListener('click', eat, { capture: true }), 400)
    const dx = e.clientX - s.x
    // A flick: fast at release, in the turning direction, and not a nudge.
    const flick = Math.abs(g.v) > 0.6 && Math.sign(-g.v) === g.d && Math.abs(dx) > 40 && e.timeStamp - g.lastT < 80
    const turn = Math.abs(dx) > g.w * 0.25 || flick
    const x = g.d > 0 ? g.w + dx : dx
    const f = g.f
    busy = true
    if (reduced()) {
      f.el.remove()
      if (turn) await o.onswipe(g.d)
      busy = false
      return
    }
    if (turn) {
      // Finish the fold to the far side, then show the new page.
      await animate(x, g.d > 0 ? -g.w : 2 * g.w, 220, (v) => place(f, g.w, v, g.d))
      await o.onswipe(g.d)
      await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))
    } else {
      await animate(x, g.d > 0 ? g.w : 0, 180, (v) => place(f, g.w, v, g.d))
    }
    f.el.remove()
    busy = false
  }
  const cancel = () => {
    start = null
    drag?.f?.el.remove()
    node.style.transform = ''
    drag = null
  }
  node.addEventListener('pointerdown', down)
  node.addEventListener('pointermove', move)
  node.addEventListener('pointerup', up)
  node.addEventListener('pointercancel', cancel)
  return {
    update(n: SwipeOpts) {
      o = n
    },
    destroy() {
      node.removeEventListener('pointerdown', down)
      node.removeEventListener('pointermove', move)
      node.removeEventListener('pointerup', up)
      node.removeEventListener('pointercancel', cancel)
    },
  }
}
