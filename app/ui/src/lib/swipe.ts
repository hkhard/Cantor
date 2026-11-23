// PERF-05: swipe left/right to turn a page, with a quick page-fold
// animation. A Svelte action on the score pane. Transforms only (60 fps),
// none at all with prefers-reduced-motion.
export interface SwipeOpts {
  enabled: boolean
  /** +1 = next (swipe to the left), -1 = previous. */
  onswipe: (d: number) => void | Promise<void>
}

const reduced = () => typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches

/** Fold the pane out around one edge, turn, and unfold from the other. */
async function foldTurn(node: HTMLElement, d: number, turn: () => void | Promise<void>) {
  if (reduced() || !node.animate) {
    await turn()
    return
  }
  const p = 'perspective(1400px)'
  const out = node.animate(
    [
      { transform: `${p} rotateY(0deg)`, transformOrigin: d > 0 ? 'left center' : 'right center' },
      { transform: `${p} rotateY(${d > 0 ? -75 : 75}deg)`, transformOrigin: d > 0 ? 'left center' : 'right center' },
    ],
    { duration: 130, easing: 'cubic-bezier(0.4, 0, 1, 1)' },
  )
  await out.finished.catch(() => {})
  await turn()
  node.animate(
    [
      { transform: `${p} rotateY(${d > 0 ? 75 : -75}deg)`, transformOrigin: d > 0 ? 'right center' : 'left center' },
      { transform: `${p} rotateY(0deg)`, transformOrigin: d > 0 ? 'right center' : 'left center' },
    ],
    { duration: 170, easing: 'cubic-bezier(0, 0, 0.2, 1)' },
  )
}

export function swipe(node: HTMLElement, opts: SwipeOpts) {
  let o = opts
  let start: { x: number; y: number; t: number; id: number } | null = null
  const down = (e: PointerEvent) => {
    if (!o.enabled || !e.isPrimary) return
    start = { x: e.clientX, y: e.clientY, t: performance.now(), id: e.pointerId }
  }
  const up = (e: PointerEvent) => {
    const s = start
    start = null
    if (!s || !o.enabled || e.pointerId !== s.id) return
    const dx = e.clientX - s.x
    const dy = e.clientY - s.y
    // A deliberate horizontal flick: far enough, mostly sideways, quick.
    if (Math.abs(dx) < 60 || Math.abs(dx) < Math.abs(dy) * 1.5 || performance.now() - s.t > 700) return
    // The release must not also count as a tap on a bar.
    const eat = (c: Event) => {
      c.stopPropagation()
      c.preventDefault()
    }
    node.addEventListener('click', eat, { capture: true, once: true })
    setTimeout(() => node.removeEventListener('click', eat, { capture: true }), 400)
    const d = dx < 0 ? 1 : -1
    foldTurn(node, d, () => o.onswipe(d))
  }
  const cancel = () => (start = null)
  node.addEventListener('pointerdown', down)
  node.addEventListener('pointerup', up)
  node.addEventListener('pointercancel', cancel)
  return {
    update(n: SwipeOpts) {
      o = n
    },
    destroy() {
      node.removeEventListener('pointerdown', down)
      node.removeEventListener('pointerup', up)
      node.removeEventListener('pointercancel', cancel)
    },
  }
}
