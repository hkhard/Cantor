// Types mirroring crates/annotations (serde's default externally tagged JSON).
import { invoke } from '@tauri-apps/api/core'

export type Tool = 'pen' | 'highlighter' | 'eraser' | 'text' | 'stamp'

export type Anchor =
  | { Page: { page: number; x: number; y: number } }
  | { Measure: { measure: number; staff: number | null; dx: number; dy: number } }

export type AnnItem =
  | { Ink: { points: { x: number; y: number; pressure: number }[]; color: string; width: number; tool: 'Pen' | 'Highlighter' } }
  | { Text: { text: string; size: number; color: string } }
  | { Stamp: { kind: unknown; scale: number } }
  | { Shape: { kind: string; w: number; h: number; color: string } }
  | { Raster: { page: number; png_bytes: number[]; png_base64?: string } }

export interface Annotation {
  id: number
  layer: number
  anchor: Anchor
  item: AnnItem
}

export interface AnnDoc {
  version: number
  layers: { name: string; visible: boolean }[]
  items: Annotation[]
  next_id: number
}

export type AnnCommand =
  | { op: 'Add'; layer: number; anchor: Anchor; item: AnnItem }
  | { op: 'Remove'; id: number }
  | { op: 'RemoveMany'; ids: number[] }
  | { op: 'Erase'; page: number; points: [number, number][]; radius: number }
  | { op: 'AddLayer'; name: string }
  | { op: 'SetLayerVisible'; layer: number; visible: boolean }

/** A measure's box on a page, in page fractions (pages 1-based). */
export interface Box {
  page: number
  x: number
  y: number
  w: number
  h: number
}

/** Mirror of `cantor_annotations::placed_on_page`: the page, origin and the
 *  item in page units. Measure-anchored items are stored in units of their
 *  bar's box (x by width, y and sizes by height), so they follow the bar. */
export function placeOnPage(
  a: Annotation,
  boxes: (Box | null)[] | null | undefined,
): { page: number; x: number; y: number; item: AnnItem } | null {
  if ('Page' in a.anchor) return { ...a.anchor.Page, item: a.item }
  const m = a.anchor.Measure
  const b = boxes?.[m.measure]
  if (!b) return null
  const sx = b.w
  const sy = b.h
  const it = a.item
  let item: AnnItem = it
  if ('Ink' in it)
    item = {
      Ink: {
        ...it.Ink,
        points: it.Ink.points.map((p) => ({ x: p.x * sx, y: p.y * sy, pressure: p.pressure })),
        width: it.Ink.width * sy,
      },
    }
  else if ('Text' in it) item = { Text: { ...it.Text, size: it.Text.size * sy } }
  else if ('Stamp' in it) item = { Stamp: { ...it.Stamp, scale: it.Stamp.scale * sy } }
  else if ('Shape' in it) item = { Shape: { ...it.Shape, w: it.Shape.w * sx, h: it.Shape.h * sy } }
  return { page: b.page, x: b.x + m.dx * sx, y: b.y + m.dy * sy, item }
}

export const STAMPS: { id: string; label: string; name: string }[] = [
  { id: 'Breath', label: '∨', name: 'stamp.Breath' },
  { id: 'Fermata', label: '𝄐', name: 'stamp.Fermata' },
  { id: 'Pp', label: 'pp', name: 'pp, pianissimo' },
  { id: 'P', label: 'p', name: 'p, piano' },
  { id: 'Mp', label: 'mp', name: 'mp, mezzo piano' },
  { id: 'Mf', label: 'mf', name: 'mf, mezzo forte' },
  { id: 'F', label: 'f', name: 'f, forte' },
  { id: 'Ff', label: 'ff', name: 'ff, fortissimo' },
  { id: 'Crescendo', label: '<', name: 'crescendo' },
  { id: 'Diminuendo', label: '>', name: 'diminuendo' },
  { id: '1', label: '1', name: '1' },
  { id: '2', label: '2', name: '2' },
  { id: '3', label: '3', name: '3' },
]

export function glyph(kind: unknown): string {
  if (typeof kind === 'string') return STAMPS.find((s) => s.id === kind)?.label ?? kind
  const k = kind as { Fingering?: number; Custom?: string }
  if (k.Fingering !== undefined) return String(k.Fingering)
  return k.Custom ?? '?'
}

/** Raster layers arrive as byte arrays; turn them into base64 once. */
function prepare(doc: AnnDoc): AnnDoc {
  for (const a of doc.items) {
    if ('Raster' in a.item && !a.item.Raster.png_base64) {
      const b = a.item.Raster.png_bytes
      let s = ''
      for (let i = 0; i < b.length; i += 0x8000) s += String.fromCharCode(...b.slice(i, i + 0x8000))
      a.item.Raster.png_base64 = btoa(s)
    }
  }
  return doc
}

export async function openAnnotations(file: string): Promise<AnnDoc> {
  return prepare(await invoke<AnnDoc>('annotations_open', { file }))
}

export async function editAnnotations(action: 'apply' | 'undo' | 'redo', command?: AnnCommand): Promise<AnnDoc> {
  return prepare(await invoke<AnnDoc>('annotations_edit', { action, command: command ?? null }))
}
