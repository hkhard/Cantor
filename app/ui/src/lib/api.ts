// Thin typed wrapper over the Tauri commands in app/src-tauri/src/lib.rs.
import { invoke } from '@tauri-apps/api/core'

export interface ScoreEntry {
  file: string
  title: string
}

export interface ChannelInfo {
  label: string
  role: string
  program: number
  instrument: string
  pan: number
}

export interface Opened {
  title: string
  musicxml: string
  channels: ChannelInfo[]
  /** [start seconds, written measure index] per played measure */
  measures: [number, number][]
  duration: number
  choir: 'male' | 'female' | 'mixed' | 'auto'
  sample_rate: number
  /** Original PDF file name, served over the cantor:// scheme. */
  pdf_url: string | null
  /** Written measure -> box on the PDF page, when the map matches. */
  measure_boxes: { page: number; x: number; y: number; w: number; h: number }[] | null
}

export interface Status {
  playing: boolean
  position: number
  duration: number
  tempo: number
}

export interface Strip {
  volume_db: number
  mute: boolean
  solo: boolean
}

export const api = {
  listScores: () => invoke<ScoreEntry[]>('list_scores'),
  openScore: (file: string, choir?: string) => invoke<Opened>('open_score', { file, choir }),
  play: () => invoke<void>('play'),
  pause: () => invoke<void>('pause'),
  seek: (seconds: number) => invoke<void>('seek', { seconds }),
  setTempo: (scale: number) => invoke<void>('set_tempo', { scale }),
  status: () => invoke<Status>('status'),
  setChannel: (index: number, c: { volumeDb?: number; mute?: boolean; solo?: boolean }) =>
    invoke<void>('set_channel', { index, volumeDb: c.volumeDb, mute: c.mute, solo: c.solo }),
  applyPreset: (preset: 'my_part' | 'without_me' | 'balanced', index?: number) =>
    invoke<Strip[]>('apply_preset', { preset, index }),
}
