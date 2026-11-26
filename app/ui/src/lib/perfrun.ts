// NFR-03 performance self-test, run on a real device: the backend answers
// `perf_selftest` with a request only when the app was launched with
// CHOIRBIRD_PERF_SELFTEST=1 (devicectl/simctl), so users never see it.
// It drives the real UI with ordinary DOM events and measures every frame
// with requestAnimationFrame, then writes the numbers to Documents/.perf.log
// through `perf_mark` (name, value, memory).
//
// Measured: library scroll, opening a piece (until the first page shows),
// PDF page turns by tap, page turns by swipe (the book-like curl), the
// mixer scroll, and the time from launch to the library.
import { api } from './api'

type Stats = { frames: number; p50: number; p95: number; max: number; dropped: number; fps: number }

const raf = () => new Promise<number>((r) => requestAnimationFrame(r))
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms))

/** Frame times while `work` runs (plus a short tail), in ms. */
async function frames(work: () => Promise<void>): Promise<Stats> {
  const times: number[] = []
  let last = await raf()
  let on = true
  const loop = async () => {
    while (on) {
      const t = await raf()
      times.push(t - last)
      last = t
    }
  }
  const l = loop()
  await work()
  await wait(250)
  on = false
  await l
  return stats(times)
}

function stats(ms: number[]): Stats {
  const s = [...ms].sort((a, b) => a - b)
  const q = (p: number) => s[Math.min(s.length - 1, Math.floor(p * s.length))] ?? 0
  // The display refresh: the most common frame interval (60 or 120 Hz).
  const base = q(0.5) || 16.7
  // A frame counts as dropped for each whole refresh interval it ran over.
  const dropped = ms.reduce((n, t) => n + Math.max(0, Math.round(t / base) - 1), 0)
  const total = ms.reduce((a, b) => a + b, 0)
  return { frames: ms.length, p50: q(0.5), p95: q(0.95), max: s[s.length - 1] ?? 0, dropped, fps: total ? (ms.length * 1000) / total : 0 }
}

async function report(name: string, s: Stats) {
  const line = `${name}: ${s.frames} frames, ${s.fps.toFixed(1)} fps, p50 ${s.p50.toFixed(1)} ms, p95 ${s.p95.toFixed(1)} ms, max ${s.max.toFixed(1)} ms, dropped ${s.dropped}`
  await api.perfMark(`perf ${line}`, s.p95).catch(() => {})
}

/** A smooth finger-like scroll of `el` by `px` over `ms`. */
async function scrollBy(el: Element, px: number, ms: number) {
  const t0 = performance.now()
  const y0 = el.scrollTop
  for (;;) {
    const k = Math.min(1, (performance.now() - t0) / ms)
    el.scrollTop = y0 + px * k
    if (k >= 1) break
    await raf()
  }
}

/** A swipe from right to left across `el` with pointer events (page curl). */
async function swipeLeft(el: HTMLElement) {
  const r = el.getBoundingClientRect()
  const y = r.top + r.height * 0.5
  const x0 = r.left + r.width * 0.85
  const x1 = r.left + r.width * 0.15
  const o = { pointerId: 7, isPrimary: true, pointerType: 'touch', bubbles: true }
  el.dispatchEvent(new PointerEvent('pointerdown', { ...o, clientX: x0, clientY: y }))
  const steps = 18
  for (let i = 1; i <= steps; i++) {
    await raf()
    el.dispatchEvent(new PointerEvent('pointermove', { ...o, clientX: x0 + ((x1 - x0) * i) / steps, clientY: y }))
  }
  el.dispatchEvent(new PointerEvent('pointerup', { ...o, clientX: x1, clientY: y }))
  // The curl finishes (220 ms) and the new page is shown.
  await wait(600)
}

async function until<T>(f: () => T | null | undefined, ms = 30000): Promise<T> {
  const t0 = performance.now()
  for (;;) {
    const v = f()
    if (v) return v
    if (performance.now() - t0 > ms) throw new Error('timeout')
    await wait(50)
  }
}

export async function perfRun(launchMs: number) {
  const q = <T extends Element>(s: string) => document.querySelector<T>(s)
  await api.perfMark('perf launch to library', launchMs).catch(() => {})
  await until(() => !q('.splash'))
  await wait(800)
  // Library scroll (down and back).
  const lib = await until(() => q<HTMLElement>('.library'))
  await report('library scroll', await frames(async () => {
    await scrollBy(lib, Math.max(400, lib.scrollHeight - lib.clientHeight), 1200)
    await scrollBy(lib, -lib.scrollTop, 1200)
  }))
  // Open the demo (Lacrymosa) and time the first page.
  const card = await until(() =>
    [...document.querySelectorAll<HTMLElement>('ul.scores > li, button.trycard')].find((e) => /Lacrymosa/.test(e.textContent ?? '')),
  )
  const t0 = performance.now()
  const open = card.querySelector<HTMLElement>('button.open') ?? card
  const openStats = await frames(async () => {
    open.click()
    await until(() => q('.pdfview canvas, .score svg'))
    await raf()
  })
  await api.perfMark('perf open Lacrymosa to first page', performance.now() - t0).catch(() => {})
  await report('open Lacrymosa', openStats)
  await wait(1500)
  const hint = q<HTMLElement>('.readerhint button.primary')
  hint?.click()
  // Page turns by tap (right edge), then back.
  const right = await until(() => q<HTMLElement>('button.pageturn.right'))
  const left = q<HTMLElement>('button.pageturn.left')
  await report('page turn tap x8', await frames(async () => {
    for (let i = 0; i < 4; i++) {
      right.click()
      await wait(450)
    }
    for (let i = 0; i < 4; i++) {
      left?.click()
      await wait(450)
    }
  }))
  // Page turns by swipe (the curl).
  const pane = await until(() => q<HTMLElement>('.score.pdf, .score'))
  await report('page turn swipe x4', await frames(async () => {
    for (let i = 0; i < 4; i++) await swipeLeft(pane)
  }))
  // The mixer: open it if closed and scroll it.
  if (!q('.mixer')) q<HTMLElement>('button[aria-pressed][aria-label] svg')?.closest('button')?.click()
  const mixer = await until(() => q<HTMLElement>('.mixer'), 3000).catch(() => null)
  if (mixer) {
    await report('mixer scroll', await frames(async () => {
      await scrollBy(mixer, Math.max(200, mixer.scrollHeight - mixer.clientHeight), 800)
      await scrollBy(mixer, -mixer.scrollTop, 800)
    }))
  }
  // LIB-04: back to the library, scroll down, open a piece in view, come
  // back: where the list lands and how long until it is there.
  q<HTMLElement>('.top > button.icon')?.click()
  const lib2 = await until(() => q<HTMLElement>('main.library'))
  await wait(1500)
  const max = lib2.scrollHeight - lib2.clientHeight
  if (max < 40) {
    await api.perfMark('perf library back: skipped (the library does not scroll)', 0).catch(() => {})
  } else {
    await scrollBy(lib2, Math.min(max, 900) - lib2.scrollTop, 600)
    await wait(500)
    const at = lib2.scrollTop
    const view = lib2.getBoundingClientRect()
    const item = [...document.querySelectorAll<HTMLElement>('ul.scores > li button.open, button.trycard')].find((b) => {
      const r = b.getBoundingClientRect()
      return r.top > view.top + 40 && r.bottom < view.bottom - 40
    })
    if (!item) {
      await api.perfMark(`perf library back: skipped (no piece in view at ${Math.round(at)})`, 0).catch(() => {})
    } else {
      item.click()
      await until(() => q('main.reader'))
      await wait(1500)
      q<HTMLElement>('.readerhint button.primary')?.click()
      await wait(300)
      const t1 = performance.now()
      let landed = -1
      let ms = 0
      const s = await frames(async () => {
        q<HTMLElement>('.top > button.icon')?.click()
        const el = await until(() => q<HTMLElement>('main.library'))
        for (;;) {
          await raf()
          if (Math.abs(el.scrollTop - at) < 2) break
          if (performance.now() - t1 > 3000) break
        }
        ms = performance.now() - t1
        await wait(500)
        landed = el.scrollTop
      })
      await api
        .perfMark(`perf library back: was ${Math.round(at)}, back at ${Math.round(landed)}, in place after ${Math.round(ms)} ms, dropped ${s.dropped}`, Math.abs(landed - at))
        .catch(() => {})
    }
  }
  await api.perfMark('perf done', 0).catch(() => {})
}
