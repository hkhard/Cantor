// NFR-10: Swedish and English UI strings. The language follows the device
// (navigator.language) unless the user picks one; the choice is kept in
// localStorage. `t()` reads reactive state, so switching re-renders.

const sv = {
  'common.cancel': 'Avbryt',
  'common.save': 'Spara',
  'common.close': 'Stäng',
  'common.delete': 'Ta bort',
  'common.done': 'Klar',
  'lib.subtitle': 'Noter som spelar alla stämmor',
  'lib.import': '＋ Importera',
  'lib.importing': 'Importerar …',
  'lib.imported': 'Importerade {n} fil(er)',
  'lib.search': 'Sök titel, kompositör, tagg …',
  'lib.all': 'Alla noter',
  'lib.setlists': 'Listor',
  'lib.newSetlist': '＋ Ny lista',
  'lib.setlistName': 'Namn på listan',
  'lib.plays': '♪ spelar',
  'lib.mine': 'Mina',
  'lib.edit': 'Info',
  'lib.removeConfirm': 'Ta bort "{title}" från ditt bibliotek?',
  'lib.note': 'Sjungen text finns bara på engelska. Svensk text visas i noterna.',
  'lib.noHits': 'Inga träffar.',
  'lib.about': 'Om Cantor',
  'set.add': 'Lägg till i lista',
  'set.empty': 'Listan är tom. Lägg till stycken med ＋ i listan Alla noter.',
  'set.missing': 'saknas',
  'set.deleteConfirm': 'Ta bort listan "{name}"? Noterna finns kvar.',
  'set.up': 'Flytta upp',
  'set.down': 'Flytta ned',
  'set.remove': 'Ta bort ur listan',
  'set.playlist': 'Spela listan',
  'meta.title': 'Om stycket',
  'meta.name': 'Titel',
  'meta.composer': 'Kompositör',
  'meta.arranger': 'Arrangör',
  'meta.key': 'Tonart',
  'meta.tempo': 'Tempo (bpm)',
  'meta.genre': 'Genre',
  'meta.tags': 'Taggar',
  'meta.tagsHint': 'jul, konsert vår, a cappella',
  'meta.rating': 'Betyg',
  'meta.difficulty': 'Svårighet',
  'read.library': 'Bibliotek',
  'read.choir': 'Kör',
  'read.choirAuto': 'Auto',
  'read.choirMixed': 'Blandad',
  'read.choirMale': 'Manskör',
  'read.choirFemale': 'Damkör',
  'read.original': 'Original',
  'read.notation': 'Notbild',
  'read.annotate': 'Anteckna',
  'read.more': 'Mer',
  'read.perform': 'Framförande',
  'read.performExit': 'Avsluta framförande',
  'read.pedal': 'Pedal',
  'read.pedalHint': 'Bläddringspedaler (AirTurn, PageFlip) och tangentbord. Välj vad varje tangent gör.',
  'read.next': 'Nästa sida',
  'read.prev': 'Föregående sida',
  'read.playPause': 'Spela / paus',
  'read.nothing': 'Inget',
  'read.mixer': 'Mixer',
  'read.hideMixer': 'Dölj mixer',
  'read.appearance': 'Utseende',
  'read.transpose': 'Transponera',
  'read.semitones': '{n} halvtoner',
  'read.parts': 'Stämmor i notbilden',
  'read.pageMode': 'Sidvisning',
  'read.pageSingle': 'En sida',
  'read.pageTwo': 'Två sidor',
  'read.pageHalf': 'Halvsida',
  'read.pageScroll': 'Rulla',
  'read.opening': 'Öppnar {title} …',
  'read.engraving': 'Sätter noterna …',
  'read.pdfOnly':
    'Bara PDF: inga noter att spela. Lägg till en MusicXML-fil med samma namn (från MuseScore eller en notskanner) så spelar stycket.',
  'mix.myPart': 'Min stämma',
  'mix.withoutMe': 'Utan mig',
  'mix.balanced': 'Balans',
  'mix.hint': 'Tryck på ett namn för att välja din stämma.',
  'mix.savePreset': '＋ Spara mix',
  'mix.presetName': 'Namn på mixen',
  'mix.volume': 'Volym {label}',
  'mix.pipe': 'Ton',
  'mix.mute': 'Tysta',
  'mix.solo': 'Solo',
  'tr.start': 'Till början',
  'tr.play': 'Spela',
  'tr.pause': 'Paus',
  'tr.position': 'Position',
  'tr.tempo': 'Tempo',
  'tr.loop': 'Slinga',
  'tr.loopA': 'Tryck första takten',
  'tr.loopB': 'Tryck sista takten',
  'tr.bars': 'T. {a}–{b}',
  'tr.click': 'Klick',
  'tr.countIn': 'Inräkning',
  'tr.export': 'Exportera',
  'tr.exportMix': 'Det du hör (en fil)',
  'tr.exportParts': 'Övningsfil per stämma',
  'tr.exportingMix': 'Spelar in det du hör …',
  'tr.exportingParts': 'Spelar in en fil per stämma …',
  'tr.exported': 'Klart: {n} fil(er) i Filer › Cantor › Export',
  'tr.exportFailed': 'Export misslyckades: {e}',
  'tr.follow': 'Följ',
  'tr.pipe': 'Tonen',
  'tr.pipeHint': 'Startton för alla stämmor',
  'ann.pen': 'Penna',
  'ann.highlighter': 'Överstrykning',
  'ann.eraser': 'Sudd',
  'ann.text': 'Text',
  'ann.stamp': 'Stämpel',
  'ann.undo': 'Ångra',
  'ann.redo': 'Gör om',
  'ann.layer': 'Lager',
  'ann.newLayer': '＋ Nytt lager',
  'ann.layerName': 'Namn på lagret',
  'ann.textPrompt': 'Text',
  'ann.color': 'Färg',
  'about.title': 'Om Cantor',
  'about.body':
    'Cantor visar dina noter och spelar varje stämma med eget instrument, så att du kan öva din stämma.',
  'about.licences': 'Licenser',
  'about.language': 'Språk',
  'about.textSize': 'Textstorlek',
  'a11y.page': 'Notsida {n}',
  'a11y.notation': 'Notbild, sida {n} av {of}. Tryck på en takt för att spela därifrån.',
  'pdf.noMap': 'Takterna på sidan är inte kopplade till ljudet för det här stycket, så markören visas inte.',
  'share.title': 'Dela',
  'share.pdf': 'PDF med anteckningar',
  'share.4sc': 'forScore (.4sc)',
  'share.setlist': 'Cantor-lista med noter, anteckningar och mixar',
  'share.4ss': 'forScore-lista (.4ss)',
  'share.done': 'Delad: {name}',
  'share.failed': 'Kunde inte dela: {e}',
  'pdf.openFailed': 'Kunde inte öppna PDF: {e}',
} as const

export type Key = keyof typeof sv

const en: Record<Key, string> = {
  'common.cancel': 'Cancel',
  'common.save': 'Save',
  'common.close': 'Close',
  'common.delete': 'Delete',
  'common.done': 'Done',
  'lib.subtitle': 'Sheet music that plays every part',
  'lib.import': '＋ Import',
  'lib.importing': 'Importing …',
  'lib.imported': 'Imported {n} file(s)',
  'lib.search': 'Search title, composer, tag …',
  'lib.all': 'All scores',
  'lib.setlists': 'Setlists',
  'lib.newSetlist': '＋ New setlist',
  'lib.setlistName': 'Setlist name',
  'lib.plays': '♪ plays',
  'lib.mine': 'Mine',
  'lib.edit': 'Info',
  'lib.removeConfirm': 'Remove "{title}" from your library?',
  'lib.note': 'Sung lyrics are English only. Swedish lyrics are shown in the score.',
  'lib.noHits': 'No matches.',
  'lib.about': 'About Cantor',
  'set.add': 'Add to setlist',
  'set.empty': 'This setlist is empty. Add pieces with ＋ in All scores.',
  'set.missing': 'missing',
  'set.deleteConfirm': 'Delete the setlist "{name}"? The scores stay.',
  'set.up': 'Move up',
  'set.down': 'Move down',
  'set.remove': 'Remove from setlist',
  'set.playlist': 'Play setlist',
  'meta.title': 'About this piece',
  'meta.name': 'Title',
  'meta.composer': 'Composer',
  'meta.arranger': 'Arranger',
  'meta.key': 'Key',
  'meta.tempo': 'Tempo (bpm)',
  'meta.genre': 'Genre',
  'meta.tags': 'Tags',
  'meta.tagsHint': 'christmas, spring concert, a cappella',
  'meta.rating': 'Rating',
  'meta.difficulty': 'Difficulty',
  'read.library': 'Library',
  'read.choir': 'Choir',
  'read.choirAuto': 'Auto',
  'read.choirMixed': 'Mixed',
  'read.choirMale': 'Male',
  'read.choirFemale': 'Female',
  'read.original': 'Original',
  'read.notation': 'Notation',
  'read.annotate': 'Annotate',
  'read.more': 'More',
  'read.perform': 'Perform',
  'read.performExit': 'Exit performance',
  'read.pedal': 'Pedal',
  'read.pedalHint': 'Page-turn pedals (AirTurn, PageFlip) and keyboards. Choose what each key does.',
  'read.next': 'Next page',
  'read.prev': 'Previous page',
  'read.playPause': 'Play / pause',
  'read.nothing': 'Nothing',
  'read.mixer': 'Mixer',
  'read.hideMixer': 'Hide mixer',
  'read.appearance': 'Appearance',
  'read.transpose': 'Transpose',
  'read.semitones': '{n} semitones',
  'read.parts': 'Parts in notation',
  'read.pageMode': 'Page view',
  'read.pageSingle': 'Single',
  'read.pageTwo': 'Two pages',
  'read.pageHalf': 'Half page',
  'read.pageScroll': 'Scroll',
  'read.opening': 'Opening {title} …',
  'read.engraving': 'Engraving …',
  'read.pdfOnly':
    'PDF only: no notes to play. Add a MusicXML file with the same name (from MuseScore or a music scanner) and the piece will play.',
  'mix.myPart': 'My part',
  'mix.withoutMe': 'Without me',
  'mix.balanced': 'Balanced',
  'mix.hint': 'Tap a name to choose your part.',
  'mix.savePreset': '＋ Save mix',
  'mix.presetName': 'Mix name',
  'mix.volume': 'Volume {label}',
  'mix.pipe': 'Note',
  'mix.mute': 'Mute',
  'mix.solo': 'Solo',
  'tr.start': 'To start',
  'tr.play': 'Play',
  'tr.pause': 'Pause',
  'tr.position': 'Position',
  'tr.tempo': 'Tempo',
  'tr.loop': 'Loop',
  'tr.loopA': 'Tap the first bar',
  'tr.loopB': 'Tap the last bar',
  'tr.bars': 'Bars {a}–{b}',
  'tr.click': 'Click',
  'tr.countIn': 'Count-in',
  'tr.export': 'Export',
  'tr.exportMix': 'What you hear (one file)',
  'tr.exportParts': 'Practice track per part',
  'tr.exportingMix': 'Recording what you hear …',
  'tr.exportingParts': 'Recording one file per part …',
  'tr.exported': 'Done: {n} file(s) in Files › Cantor › Export',
  'tr.exportFailed': 'Export failed: {e}',
  'tr.follow': 'Follow',
  'tr.pipe': 'Pitch',
  'tr.pipeHint': 'Starting note for all parts',
  'ann.pen': 'Pen',
  'ann.highlighter': 'Highlighter',
  'ann.eraser': 'Eraser',
  'ann.text': 'Text',
  'ann.stamp': 'Stamp',
  'ann.undo': 'Undo',
  'ann.redo': 'Redo',
  'ann.layer': 'Layer',
  'ann.newLayer': '＋ New layer',
  'ann.layerName': 'Layer name',
  'ann.textPrompt': 'Text',
  'ann.color': 'Colour',
  'about.title': 'About Cantor',
  'about.body': 'Cantor shows your scores and plays every part with its own instrument, so you can rehearse your line.',
  'about.licences': 'Licences',
  'about.language': 'Language',
  'about.textSize': 'Text size',
  'a11y.page': 'Score page {n}',
  'a11y.notation': 'Notation, page {n} of {of}. Tap a bar to play from there.',
  'pdf.noMap': "This page's bars are not linked to the audio for this piece, so the cursor is not shown.",
  'share.title': 'Share',
  'share.pdf': 'PDF with annotations',
  'share.4sc': 'forScore (.4sc)',
  'share.setlist': 'Cantor setlist with scores, annotations and mixes',
  'share.4ss': 'forScore setlist (.4ss)',
  'share.done': 'Shared: {name}',
  'share.failed': 'Could not share: {e}',
  'pdf.openFailed': 'Could not open PDF: {e}',
}

export type Lang = 'sv' | 'en'
const dicts: Record<Lang, Record<Key, string>> = { sv, en }

function initial(): Lang {
  const saved = typeof localStorage !== 'undefined' ? localStorage.getItem('cantor.lang') : null
  if (saved === 'sv' || saved === 'en') return saved
  const nav = typeof navigator !== 'undefined' ? navigator.language.toLowerCase() : 'sv'
  return nav.startsWith('sv') || nav.startsWith('nb') || nav.startsWith('da') ? 'sv' : 'en'
}

export const i18n = $state({ lang: initial() })

export function setLang(l: Lang) {
  i18n.lang = l
  localStorage.setItem('cantor.lang', l)
  document.documentElement.lang = l
}

export function t(key: Key, vars?: Record<string, string | number>): string {
  let s = dicts[i18n.lang][key] ?? sv[key] ?? key
  if (vars) for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, String(v))
  return s
}

/** NFR-08 text size: the iOS system text size (Dynamic Type, read through the
 *  -apple-system-body font) times the user's own choice in About. */
export const textSize = $state({ scale: Number(localStorage.getItem('cantor.textScale') ?? '1') || 1 })

export function applyTextSize() {
  const probe = document.createElement('span')
  probe.style.font = '-apple-system-body'
  probe.style.position = 'absolute'
  probe.style.visibility = 'hidden'
  document.body.appendChild(probe)
  const sys = parseFloat(getComputedStyle(probe).fontSize)
  probe.remove()
  // iOS reports 17px for -apple-system-body at the default text size (Large)
  // and more or less as the user changes Dynamic Type; keep the UI's 16px
  // base at the default. Elsewhere (desktop WebKit says 13px) use 16px.
  const ios = /iPad|iPhone/.test(navigator.userAgent) || (navigator.maxTouchPoints > 1 && /Macintosh/.test(navigator.userAgent))
  const base = ios && sys && Number.isFinite(sys) ? Math.max(12, Math.min(32, (sys / 17) * 16)) : 16
  document.documentElement.style.setProperty('--system-text', `${base}px`)
  document.documentElement.style.setProperty('--text-scale', String(textSize.scale))
}

export function setTextScale(s: number) {
  textSize.scale = s
  localStorage.setItem('cantor.textScale', String(s))
  applyTextSize()
}
