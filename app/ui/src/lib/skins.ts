// Skinnable design system. A skin is a flat map of semantic tokens applied as
// CSS custom properties on <html>. Components only use tokens (var(--x)),
// never raw colours, so adding a skin = adding an entry to SKINS.
//
// Preference is 'system' or a skin id. 'system' follows the OS appearance
// (prefers-color-scheme) live, using the skins named in SYSTEM_MAP.

export type Scheme = 'light' | 'dark'

export interface Skin {
  id: string
  name: string
  scheme: Scheme
  tokens: Record<string, string>
}

// Brand palette from the ChoirBird "Songbird" icon (design/choirbird,
// 1-Songbird): navy sky, coral bird, peach wing, amber beak, cream song.
export const brand = {
  navy: '#1E3A7B',
  navyDeep: '#0D1B3E',
  night: '#0E1424',
  cream: '#F4EBDD',
  amber: '#F2A541',
  coral: '#FF7A5C',
  peach: '#FFB4A2',
  // Coral deep enough for text and white labels on light paper (AA).
  coralInk: '#B83E26',
}

/** Tokens shared by every skin (shape, type, motion). */
const base: Record<string, string> = {
  'font-ui': "-apple-system, 'SF Pro Text', system-ui, sans-serif",
  'font-display': "'New York', 'Iowan Old Style', Georgia, serif",
  'radius-s': '10px',
  'radius-m': '14px',
  'radius-l': '22px',
  'radius-pill': '999px',
  'glass-blur': '24px',
  'glass-saturate': '180%',
  ease: 'cubic-bezier(0.2, 0.8, 0.2, 1)',
}

const light: Skin = {
  id: 'light',
  name: 'Ljus',
  scheme: 'light',
  tokens: {
    // Warm cream with a hint of the bird's peach, navy text (the icon's sky).
    bg: '#F7F2EA',
    'bg-gradient': 'radial-gradient(120% 80% at 0% 0%, #FFFFFF 0%, #F7F2EA 55%, #F1E6DA 100%)',
    surface: '#FFFFFF',
    ink: '#14213D',
    // WCAG 2.2 AA (NFR-08): muted text >= 4.5:1 on the background, white on
    // the accent >= 4.5:1 (was #6B6F7B / #C77A12 at 4.4 / 3.4).
    'ink-muted': '#5D616C',
    line: 'rgba(20, 33, 61, 0.12)',
    // Accent: the bird's coral, deepened for AA: 5.0:1 as text on the
    // background, white on it 5.6:1.
    accent: brand.coralInk,
    'accent-ink': '#FFFFFF',
    'accent-soft': 'rgba(255, 122, 92, 0.14)',
    brand: brand.navy,
    danger: '#B91C1C',
    'glass-bg': 'rgba(255, 255, 255, 0.62)',
    'glass-bg-strong': 'rgba(255, 255, 255, 0.82)',
    'glass-border': 'rgba(255, 255, 255, 0.9)',
    'glass-edge': 'rgba(20, 33, 61, 0.08)',
    'glass-highlight': 'rgba(255, 255, 255, 0.95)',
    'glass-shadow': '0 8px 32px rgba(20, 33, 61, 0.12), 0 1px 2px rgba(20, 33, 61, 0.08)',
    control: 'rgba(20, 33, 61, 0.06)',
    'control-hover': 'rgba(20, 33, 61, 0.1)',
    // VIEW-06: music is black on white, nothing in between (21:1).
    paper: '#FFFFFF',
    'paper-ink': '#000000',
    'paper-shadow': '0 2px 12px rgba(20, 33, 61, 0.10)',
    // The cursor tints the bar but leaves the notes black under it.
    'cursor-fill': 'rgba(242, 165, 65, 0.16)',
    'cursor-stroke': 'rgba(154, 52, 18, 0.75)',
    // Playing notes: dark rust, 7.3:1 on white (was #C77A12 at 3.4).
    'note-playing': '#9A3412',
    'mute-on': '#FCD34D',
    'solo-on': '#86EFAC',
    'on-toggle-ink': '#14213D',
    soprano: '#C2410C',
    alto: '#A16207',
    tenor: '#15803D',
    bass: '#1D4ED8',
    voice: '#7C3AED',
    piano: '#475569',
  },
}

const dark: Skin = {
  id: 'dark',
  name: 'Mörk',
  scheme: 'dark',
  tokens: {
    bg: brand.night,
    'bg-gradient': `radial-gradient(120% 80% at 0% 0%, ${brand.navy} 0%, ${brand.navyDeep} 45%, ${brand.night} 100%)`,
    surface: '#18213A',
    ink: brand.cream,
    'ink-muted': '#9AA3B8',
    line: 'rgba(244, 235, 221, 0.12)',
    // The icon's coral on the night sky, 7.2:1; dark ink on it 7.2:1.
    accent: brand.coral,
    'accent-ink': '#1A0B06',
    'accent-soft': 'rgba(255, 122, 92, 0.18)',
    brand: brand.coral,
    danger: '#F87171',
    'glass-bg': 'rgba(24, 33, 58, 0.55)',
    'glass-bg-strong': 'rgba(24, 33, 58, 0.82)',
    'glass-border': 'rgba(255, 255, 255, 0.14)',
    'glass-edge': 'rgba(0, 0, 0, 0.35)',
    'glass-highlight': 'rgba(255, 255, 255, 0.18)',
    'glass-shadow': '0 10px 36px rgba(0, 0, 0, 0.45), 0 1px 2px rgba(0, 0, 0, 0.4)',
    control: 'rgba(244, 235, 221, 0.08)',
    'control-hover': 'rgba(244, 235, 221, 0.14)',
    // VIEW-06: night paper is black with light grey engraving (16.8:1);
    // not navy, not cream: colour casts cost contrast and read as a tint.
    paper: '#000000',
    'paper-ink': '#E6E6E6',
    'paper-shadow': '0 2px 18px rgba(0, 0, 0, 0.5)',
    'cursor-fill': 'rgba(242, 165, 65, 0.14)',
    'cursor-stroke': 'rgba(242, 165, 65, 0.75)',
    // Amber on black, 10.2:1.
    'note-playing': brand.amber,
    'mute-on': '#B45309',
    'solo-on': '#15803D',
    'on-toggle-ink': '#FFFFFF',
    soprano: '#FB923C',
    alto: '#FACC15',
    tenor: '#4ADE80',
    bass: '#60A5FA',
    voice: '#C084FC',
    piano: '#94A3B8',
  },
}

const sepia: Skin = {
  id: 'sepia',
  name: 'Sepia',
  scheme: 'light',
  tokens: {
    ...light.tokens,
    bg: '#EFE6D4',
    'bg-gradient': 'radial-gradient(120% 80% at 0% 0%, #F8F1E3 0%, #EFE6D4 55%, #E4D8C0 100%)',
    surface: '#FAF4E8',
    ink: '#3B2F22',
    'ink-muted': '#6A5C4B',
    line: 'rgba(59, 47, 34, 0.14)',
    'glass-bg': 'rgba(250, 244, 232, 0.66)',
    'glass-bg-strong': 'rgba(250, 244, 232, 0.86)',
    // Coral accent, deeper still on sepia: 5.4:1 as text, white on it 6.5.
    accent: '#A8381F',
    'accent-soft': 'rgba(255, 122, 92, 0.14)',
    // VIEW-06: music is black on sepia paper (17.9:1), not brown on cream.
    paper: '#F5ECD7',
    'paper-ink': '#000000',
    'note-playing': '#9A3412',
  },
}

export const SKINS: Skin[] = [light, dark, sepia]
export const SYSTEM_MAP: Record<Scheme, string> = { light: 'light', dark: 'dark' }

export type Preference = 'system' | string
const KEY = 'cantor.skin'

const media = typeof matchMedia !== 'undefined' ? matchMedia('(prefers-color-scheme: dark)') : null

export function systemScheme(): Scheme {
  return media?.matches ? 'dark' : 'light'
}

export function resolve(pref: Preference): Skin {
  const id = pref === 'system' ? SYSTEM_MAP[systemScheme()] : pref
  return SKINS.find((s) => s.id === id) ?? light
}

export function applySkin(skin: Skin, root: HTMLElement = document.documentElement) {
  for (const [k, v] of Object.entries({ ...base, ...skin.tokens })) root.style.setProperty(`--${k}`, v)
  root.dataset.skin = skin.id
  root.dataset.scheme = skin.scheme
  root.style.colorScheme = skin.scheme
  document.querySelector('meta[name="theme-color"]')?.setAttribute('content', skin.tokens.bg)
}

export function loadPreference(): Preference {
  try {
    const v = localStorage.getItem(KEY)
    if (v === 'system' || SKINS.some((s) => s.id === v)) return v as Preference
  } catch {
    /* storage unavailable */
  }
  return 'system'
}

let current: Preference = 'system'
const listeners = new Set<(p: Preference, s: Skin) => void>()

export function setPreference(pref: Preference) {
  current = pref
  try {
    localStorage.setItem(KEY, pref)
  } catch {
    /* ignore */
  }
  const skin = resolve(pref)
  applySkin(skin)
  listeners.forEach((l) => l(pref, skin))
}

export function getPreference(): Preference {
  return current
}

export function onSkinChange(fn: (p: Preference, s: Skin) => void): () => void {
  listeners.add(fn)
  return () => listeners.delete(fn)
}

/** Call once before mounting: applies the stored skin and tracks OS changes. */
export function initSkins() {
  setPreference(loadPreference())
  media?.addEventListener('change', () => {
    if (current === 'system') setPreference('system')
  })
}
