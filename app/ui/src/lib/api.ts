// Thin typed wrapper over the Tauri commands in app/src-tauri/src/lib.rs.
import { invoke } from '@tauri-apps/api/core'

export interface ScoreEntry {
  /** Score id "<dir>:<stem>" (0 = bundled demo, 1 = your library). */
  file: string
  title: string
  /** Has MusicXML, so it plays. */
  playable: boolean
  /** Has the original PDF. */
  pdf: boolean
  /** Imported by you (can be removed). */
  user: boolean
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

export interface Metadata {
  title: string
  composer: string
  arranger: string
  key: string
  tempo: number | null
  genre: string
  tags: string[]
  rating: number | null
  difficulty: number | null
  part_names: string[]
}

export type SetItem =
  | { kind: 'score'; score: string; start_page: number | null }
  | { kind: 'placeholder'; title: string }

export interface Setlist {
  id: string
  name: string
  items: SetItem[]
}

export interface Preset {
  name: string
  strips: Strip[]
}

export interface ScoreSettings {
  my_part: number | null
  strips: Strip[]
  tempo_percent: number
  view: 'pdf' | 'notation'
  choir: 'auto' | 'male' | 'female' | 'mixed'
  presets: Preset[]
  transpose: number
  hidden_parts: number[]
}

export function emptyMetadata(title = ''): Metadata {
  return { title, composer: '', arranger: '', key: '', tempo: null, genre: '', tags: [], rating: null, difficulty: null, part_names: [] }
}

export const api = {
  listScores: () => invoke<ScoreEntry[]>('list_scores'),
  openScore: (file: string, choir?: string, transpose?: number) =>
    invoke<Opened>('open_score', { file, choir, transpose }),
  libraryData: () => invoke<{ metadata: Record<string, Metadata>; setlists: Setlist[] }>('library_data'),
  setMetadata: (id: string, metadata: Metadata) => invoke<void>('set_metadata', { id, metadata }),
  search: (query: string) => invoke<string[]>('search_scores', { query }),
  createSetlist: (name: string) => invoke<string>('save_setlist', { name, setlist: null }),
  saveSetlist: (setlist: Setlist) => invoke<string>('save_setlist', { setlist, name: null }),
  deleteSetlist: (id: string) => invoke<void>('delete_setlist', { id }),
  scoreSettings: (id: string) => invoke<ScoreSettings>('score_settings', { id }),
  saveScoreSettings: (id: string, settings: ScoreSettings) =>
    invoke<void>('save_score_settings', { id, settings }),
  /** PERF-03: starting note of one part, or every part as a chord. */
  /** Share: 'pdf' annotated PDF, '4sc' forScore score (id = score);
   *  'setlist' Cantor bundle, '4ss' forScore setlist (id = setlist). */
  exportItem: (kind: 'pdf' | '4sc' | 'setlist' | '4ss', id: string) => invoke<string>('export_item', { kind, id }),
  /** DATA-03: whole-library backup (share sheet) and restore. */
  backupLibrary: () => invoke<string>('backup_library'),
  restoreLibrary: async (f: File) => invoke<number>('restore_library', new Uint8Array(await f.arrayBuffer())),
  pendingBackups: () => invoke<string[]>('pending_backups'),
  restorePending: (name: string, discard = false) => invoke<number>('restore_pending', { name, discard }),
  /** NFR-03/04 probe: log a timing plus the app's memory footprint. */
  perfMark: (name: string, ms: number) =>
    invoke<{ name: string; ms: number; memory_mb: number }>('perf_mark', { name, ms }),
  pitchPipe: (channel?: number) => invoke<void>('pitch_pipe', { channel }),
  play: () => invoke<void>('play'),
  pause: () => invoke<void>('pause'),
  seek: (seconds: number) => invoke<void>('seek', { seconds }),
  setTempo: (scale: number) => invoke<void>('set_tempo', { scale }),
  status: () => invoke<Status>('status'),
  /** Import a picked file; returns the new score ids. */
  importFile: async (f: File) =>
    invoke<string[]>('import_bytes', new Uint8Array(await f.arrayBuffer()), {
      headers: { 'x-name': encodeURIComponent(f.name) },
    }),
  deleteScore: (file: string) => invoke<void>('delete_score', { file }),
  /** PLAY-11: "mix" = what you hear, "parts" = one learning track per voice. */
  exportAudio: (what: 'mix' | 'parts') => invoke<string[]>('export_audio', { what }),
  /** A-B loop over written measures a..=b (0-based); omit both to turn off. */
  setLoop: (a?: number, b?: number) => invoke<void>('set_loop', { a, b }),
  setClick: (c: { metronome?: boolean; countIn?: boolean }) =>
    invoke<void>('set_click', { metronome: c.metronome, countIn: c.countIn }),
  setChannel: (index: number, c: { volumeDb?: number; mute?: boolean; solo?: boolean }) =>
    invoke<void>('set_channel', { index, volumeDb: c.volumeDb, mute: c.mute, solo: c.solo }),
  applyPreset: (preset: 'my_part' | 'without_me' | 'balanced', index?: number) =>
    invoke<Strip[]>('apply_preset', { preset, index }),
}
