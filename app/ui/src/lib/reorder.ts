// Long-press drag to rearrange, iOS home-screen style (library grid,
// setlist tabs, setlist items).
//
// 60 fps and no glitches:
// - Nothing in the DOM changes while dragging. The dragged item follows the
//   finger with a transform; the others slide to their new slots with CSS
//   transforms (compositor only).
// - Slot geometry is measured once when the drag starts; the frame loop
//   reads no layout and writes only the transforms that changed, once per
//   requestAnimationFrame.
// - The real reorder happens once, on drop, in the same task as the
//   transforms are cleared, so the next paint shows the same picture.
// - iOS zones: the edge of another item moves it aside; dwelling on its
//   centre makes a list with it ("folder"). After a move the finger is over
//   the dragged item's own slot, so the order cannot flicker at a boundary.

export interface SortOptions {
  /** Merge onto another item by holding over its centre (library only). */
  onmerge?: (dragged: string, target: string) => void
  /** Runs synchronously in the pointerup of a merge (a user gesture), e.g.
   *  to bring up the iOS keyboard for the name prompt that follows. */
  onmergegesture?: () => void
  /** Drop onto an external target (`[data-drop]` element inside `dropRoot`). */
  ondrop?: (dragged: string, target: string) => void
  /** Root to search for `[data-drop]` targets. */
  dropRoot?: () => HTMLElement | null | undefined
  onreorder: (ids: string[]) => void
  /** Called when an item is picked up (e.g. enter edit mode). */
  onlift?: () => void
  /** In edit mode a drag starts on the first movement, without a long press. */
  immediate?: () => boolean
  disabled?: () => boolean
  /** Scrolling ancestor for edge auto-scroll. */
  scroller?: () => HTMLElement | null | undefined
  /** Drag a floating copy above everything instead of the item itself, and
   *  leave the others in place (no reordering): for dragging out of one
   *  pane onto targets in another (columns layout). The owner hit-tests:
   *  `onhover` gets the pointer every frame (viewport coordinates) and
   *  `ondropat` decides on release (true: taken). A copy-out drag always
   *  starts with a long press, also in edit mode, so a swipe scrolls. */
  copyOut?: () => boolean
  onhover?: (x: number, y: number, now: number) => void
  ondropat?: (dragged: string, x: number, y: number) => boolean
  /** The copy-out drag ended (dropped or cancelled). */
  onend?: () => void
  longPressMs?: number
}

/** Counters the UI harness reads (frames drawn, slot changes). */
export const sortStats = { reorders: 0, frames: 0 }

type Rect = { x: number; y: number; w: number; h: number }

const MOVE_CANCEL = 8 // px before the long press: it is a scroll, not a drag
const MERGE_DWELL = 450 // ms on a centre before it becomes a merge
const MOVE_DWELL = 90 // ms on an edge before the others make room (iOS-like;
// passing over a card on the way somewhere else does not shuffle it)
const EASE = 'transform 220ms cubic-bezier(0.2, 0.8, 0.2, 1)'

export function sortable(node: HTMLElement, initial: SortOptions) {
  let opts = initial
  let press: {
    id: number
    el: HTMLElement
    x: number
    y: number
    timer: number
  } | null = null
  let drag: null | {
    pointer: number
    el: HTMLElement
    items: HTMLElement[]
    ids: string[]
    slots: Rect[]
    dropTargets: { el: HTMLElement; rect: Rect }[]
    order: number[] // order[k] = original index shown in slot k
    shown: number[] // last transform slot per original index
    from: number // original index of the dragged item
    at: number // slot the dragged item occupies now
    grab: { x: number; y: number }
    px: number // pointer, client coords
    py: number
    scroller: HTMLElement | null
    merge: { id: number; since: number; on: boolean } | null
    edge: { slot: number; since: number } | null
    dropOn: HTMLElement | null
    raf: number
    ghost: HTMLElement | null
  } = null
  let suppressClick = false

  const scrollTop = () => drag?.scroller?.scrollTop ?? 0
  /** Drags start on the first movement (edit mode), never for copy-out. */
  const immediateNow = () => (opts.immediate?.() ?? false) && !(opts.copyOut?.() ?? false)

  function itemOf(t: EventTarget | null): HTMLElement | null {
    const el = (t as Element | null)?.closest?.('[data-sort-id]') as HTMLElement | null
    return el && node.contains(el) ? el : null
  }

  function down(e: PointerEvent) {
    suppressClick = false
    if (opts.disabled?.() || e.button > 0 || drag) return
    if ((e.target as Element).closest('[data-nodrag]')) return
    const el = itemOf(e.target)
    if (!el) return
    clearPress()
    const immediate = immediateNow()
    press = {
      id: e.pointerId,
      el,
      x: e.clientX,
      y: e.clientY,
      timer: immediate ? 0 : (setTimeout(() => lift(e.pointerId), opts.longPressMs ?? 420) as unknown as number),
    }
  }

  function clearPress() {
    if (press?.timer) clearTimeout(press.timer)
    press = null
  }

  function move(e: PointerEvent) {
    if (drag && e.pointerId === drag.pointer) {
      drag.px = e.clientX
      drag.py = e.clientY
      e.preventDefault()
      return
    }
    if (!press || e.pointerId !== press.id) return
    const d = Math.hypot(e.clientX - press.x, e.clientY - press.y)
    if (immediateNow()) {
      if (d > 4) lift(e.pointerId, e.clientX, e.clientY)
    } else if (d > MOVE_CANCEL) clearPress() // a scroll
  }

  function lift(pointer: number, px?: number, py?: number) {
    if (!press || press.id !== pointer) return
    const el = press.el
    const x = px ?? press.x
    const y = py ?? press.y
    clearPress()
    const items = [...node.querySelectorAll<HTMLElement>(':scope > [data-sort-id]')]
    const from = items.indexOf(el)
    if (from < 0) return
    const copy = opts.copyOut?.() ?? false
    // Copy-out: no auto-scroll of the source pane, all in viewport coordinates.
    const scroller = copy ? null : (opts.scroller?.() ?? null)
    const st = scroller?.scrollTop ?? 0
    // One measurement for the whole drag (content coordinates).
    const slots = items.map((it) => {
      const r = it.getBoundingClientRect()
      return { x: r.left, y: r.top + st, w: r.width, h: r.height }
    })
    // Copy-out targets live in other panes; with no scroller everything is
    // in viewport coordinates.
    const tst = st
    const dropTargets = [...(opts.dropRoot?.()?.querySelectorAll<HTMLElement>('[data-drop]') ?? [])].map((t) => {
      const r = t.getBoundingClientRect()
      return { el: t, rect: { x: r.left, y: r.top + tst, w: r.width, h: r.height } }
    })
    let ghost: HTMLElement | null = null
    if (copy) {
      // A floating copy on <body>, above every pane (not clipped by the
      // pane's overflow); the item stays where it is.
      const r = el.getBoundingClientRect()
      ghost = el.cloneNode(true) as HTMLElement
      ghost.classList.add('sort-ghost')
      ghost.removeAttribute('data-sort-id')
      ghost.setAttribute('aria-hidden', 'true')
      ghost.style.cssText = `position:fixed;left:${r.left}px;top:${r.top}px;width:${r.width}px;height:${r.height}px;margin:0;z-index:1000;pointer-events:none;list-style:none`
      document.body.appendChild(ghost)
    }
    drag = {
      pointer,
      el,
      items,
      ids: items.map((i) => i.dataset.sortId!),
      slots,
      dropTargets,
      order: items.map((_, i) => i),
      shown: items.map((_, i) => i),
      from,
      at: from,
      grab: { x: x - slots[from].x, y: y + st - slots[from].y },
      px: x,
      py: y,
      scroller,
      merge: null,
      edge: null,
      dropOn: null,
      raf: 0,
      ghost,
    }
    suppressClick = true
    if (ghost) {
      el.classList.add('sort-source')
      node.classList.add('sorting')
      try {
        el.setPointerCapture(pointer)
      } catch {
        /* pointer already gone */
      }
      opts.onlift?.()
      drag.raf = requestAnimationFrame(frame)
      return
    }
    // No will-change here: promoting every item to a layer in this frame
    // costs a ~30 ms stall in WebKit. Edit mode (CSS) promotes them ahead
    // of time, so drags started in edit mode pick up with no stall.
    for (const it of items) if (it !== el) it.style.transition = EASE
    el.style.transition = 'none'
    el.classList.add('sort-lifted')
    node.classList.add('sorting')
    try {
      el.setPointerCapture(pointer)
    } catch {
      /* pointer already gone */
    }
    opts.onlift?.()
    drag.raf = requestAnimationFrame(frame)
  }

  // Registered for the node's whole life (not added mid-gesture), so the
  // first touchmove after the lift can stop WebKit from starting a pan,
  // which would cancel the pointer and drop the item.
  function blockScroll(e: TouchEvent) {
    if (drag || (press && immediateNow())) e.preventDefault()
  }

  function setTransform(i: number, slot: number) {
    if (!drag || drag.shown[i] === slot) return
    drag.shown[i] = slot
    const a = drag.slots[i]
    const b = drag.slots[slot]
    drag.items[i].style.transform = slot === i ? '' : `translate3d(${b.x - a.x}px, ${b.y - a.y}px, 0)`
  }

  function frame(now: number) {
    if (!drag) return
    sortStats.frames++
    const d = drag
    // Edge auto-scroll (no layout reads: the scroller's box was measured at lift).
    if (d.scroller) {
      const h = window.innerHeight
      const speed = d.py < 90 ? -Math.ceil((90 - d.py) / 6) : d.py > h - 90 ? Math.ceil((d.py - (h - 90)) / 6) : 0
      if (speed) d.scroller.scrollTop += speed
    }
    const cy = d.py + scrollTop() // pointer in content coordinates
    const cx = d.px
    const s = d.slots[d.from]
    if (d.ghost) {
      // Copy-out: only the floating copy moves (viewport coordinates); the
      // owner hit-tests its targets (they may move: columns slide).
      d.ghost.style.transform = `translate3d(${cx - d.grab.x - s.x}px, ${d.py - d.grab.y - s.y}px, 0) scale(1.04)`
      opts.onhover?.(cx, d.py, now)
      d.raf = requestAnimationFrame(frame)
      return
    }
    d.el.style.transform = `translate3d(${cx - d.grab.x - s.x}px, ${cy - d.grab.y - s.y}px, 0) scale(1.06)`

    // External drop targets (setlist tabs).
    const over = d.dropTargets.find((t) => inside(cx, cy, t.rect))?.el ?? null
    if (over !== d.dropOn) {
      d.dropOn?.classList.remove('drop-over')
      over?.classList.add('drop-over')
      d.dropOn = over
    }
    if (!over) {
      // The slot under the finger.
      const k = d.slots.findIndex((r) => inside(cx, cy, r))
      if (k >= 0 && k !== d.at) {
        const r = d.slots[k]
        const centre = Math.abs(cx - (r.x + r.w / 2)) < r.w * 0.28 && Math.abs(cy - (r.y + r.h / 2)) < r.h * 0.32
        const target = d.order[k]
        if (centre && opts.onmerge) {
          d.edge = null // passing the edge on the way to the centre is no move
          if (!d.merge || d.merge.id !== target) {
            clearMerge()
            d.merge = { id: target, since: now, on: false }
          } else if (!d.merge.on && now - d.merge.since > MERGE_DWELL) {
            d.merge.on = true
            d.items[target].classList.add('merge-target')
          }
        } else {
          clearMerge()
          if (!d.edge || d.edge.slot !== k) d.edge = { slot: k, since: now }
          if (now - d.edge.since < (opts.onmerge ? MOVE_DWELL : 0)) {
            d.raf = requestAnimationFrame(frame)
            return
          }
          d.edge = null
          // Move the dragged item to slot k; the others shift by one slot.
          d.order.splice(d.at, 1)
          d.order.splice(k, 0, d.from)
          d.at = k
          sortStats.reorders++
          d.order.forEach((orig, slot) => orig !== d.from && setTransform(orig, slot))
        }
      } else {
        d.edge = null
        if (k === d.at) clearMerge()
      }
    } else clearMerge()
    d.raf = requestAnimationFrame(frame)
  }

  function clearMerge() {
    if (drag?.merge?.on) drag.items[drag.merge.id].classList.remove('merge-target')
    if (drag) drag.merge = null
  }

  function inside(x: number, y: number, r: Rect) {
    return x >= r.x && x <= r.x + r.w && y >= r.y && y <= r.y + r.h
  }

  function up(e: PointerEvent) {
    if (press && e.pointerId === press.id) clearPress()
    if (!drag || e.pointerId !== drag.pointer) return
    const d = drag
    cancelAnimationFrame(d.raf)
    const draggedId = d.ids[d.from]
    const merged = d.merge?.on && e.type === 'pointerup' ? d.ids[d.merge.id] : null
    if (merged) opts.onmergegesture?.()
    const dropped = e.type === 'pointerup' ? d.dropOn?.dataset.drop ?? null : null
    d.dropOn?.classList.remove('drop-over')
    if (d.ghost) {
      const g = d.ghost
      drag = null
      d.el.classList.remove('sort-source')
      node.classList.remove('sorting')
      setTimeout(() => (suppressClick = false), 0)
      const taken = e.type === 'pointerup' && (opts.ondropat?.(draggedId, d.px, d.py) ?? false)
      opts.onend?.()
      if (taken) {
        // Into the list: shrink away where it was dropped.
        g.style.transition = 'transform 180ms ease-in, opacity 180ms ease-in'
        g.style.transform += ' scale(0.6)'
        g.style.opacity = '0'
      } else {
        // Nowhere: fly back to its place.
        g.style.transition = EASE
        g.style.transform = ''
      }
      setTimeout(() => g.remove(), taken ? 180 : 230)
      return
    }
    clearMerge()
    // Settle the dragged item into its slot (slots are in content
    // coordinates, so scrolling during the drag needs no correction).
    const a = d.slots[d.from]
    const b = d.slots[d.at]
    d.el.style.transition = EASE
    d.el.style.transform = d.at === d.from ? '' : `translate3d(${b.x - a.x}px, ${b.y - a.y}px, 0)`
    d.el.classList.remove('sort-lifted')
    const newIds = d.order.map((i) => d.ids[i])
    const changed = newIds.some((id, i) => id !== d.ids[i])
    drag = null
    // The click that ends this gesture (if any) arrives right after pointerup;
    // later taps must open things normally.
    setTimeout(() => (suppressClick = false), 0)
    setTimeout(() => {
      // Same task: clear transforms and reorder; the next paint matches.
      for (const it of d.items) {
        it.style.transition = 'none'
        it.style.transform = ''
      }
      node.classList.remove('sorting')
      if (changed) opts.onreorder(newIds)
      if (merged && opts.onmerge) opts.onmerge(draggedId, merged)
      else if (dropped && opts.ondrop) opts.ondrop(draggedId, dropped)
      requestAnimationFrame(() => requestAnimationFrame(() => d.items.forEach((it) => (it.style.transition = ''))))
    }, 230)
  }

  function click(e: MouseEvent) {
    // The click that ends a long press must not open the item.
    if (suppressClick) {
      suppressClick = false
      e.stopPropagation()
      e.preventDefault()
    }
  }

  function contextmenu(e: Event) {
    if (itemOf(e.target)) e.preventDefault()
  }

  node.addEventListener('pointerdown', down)
  node.addEventListener('touchmove', blockScroll, { passive: false })
  window.addEventListener('pointermove', move, { passive: false })
  window.addEventListener('pointerup', up)
  window.addEventListener('pointercancel', up)
  node.addEventListener('click', click, true)
  node.addEventListener('contextmenu', contextmenu)
  return {
    update(o: SortOptions) {
      opts = o
    },
    destroy() {
      clearPress()
      node.removeEventListener('pointerdown', down)
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', up)
      window.removeEventListener('pointercancel', up)
      node.removeEventListener('touchmove', blockScroll)
      node.removeEventListener('click', click, true)
      node.removeEventListener('contextmenu', contextmenu)
    },
  }
}
