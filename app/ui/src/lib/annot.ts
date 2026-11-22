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
  | { op: 'Erase'; page: number; points: [number, number][]; radius: number }
  | { op: 'AddLayer'; name: string }
  | { op: 'SetLayerVisible'; layer: number; visible: boolean }

export const STAMPS: { id: string; label: string }[] = [
  { id: 'Breath', label: '∨' },
  { id: 'Fermata', label: '𝄐' },
  { id: 'Pp', label: 'pp' },
  { id: 'P', label: 'p' },
  { id: 'Mp', label: 'mp' },
  { id: 'Mf', label: 'mf' },
  { id: 'F', label: 'f' },
  { id: 'Ff', label: 'ff' },
  { id: 'Crescendo', label: '<' },
  { id: 'Diminuendo', label: '>' },
  { id: '1', label: '1' },
  { id: '2', label: '2' },
  { id: '3', label: '3' },
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
