# Reader layouts: survey of iPad score apps, options for Cantor

Date: 2026-11-22. Sources at bottom; [n] refs. Where a source didn't confirm a feature, it is marked "n/c" (not confirmed). Treat it as unknown, not absent.

## 1. Per-app summary

| App | Display modes | Page turn | Chrome | Library | Dark/paper | Playback/mixer | Choir-relevant |
|---|---|---|---|---|---|---|---|
| **forScore** (PDF) | Standard 3:4 / Best Fit / Zoomed; landscape = width-fit vertical scroll (half or third step) or Best Fit single; half-page turns (portrait, movable split); two-up..five-up by window size, 1-page or 2-page advance; Crop per page (non-destructive) [1][2][3] | L/R side taps, swipe, keys (arrows/space/PgUp), BT/MIDI pedals mappable to any action, AirPods head turn, face gestures, Replay (records turns against audio), Links (blue->orange repeat jumps), Buttons [1][4][5] | Title bar hidden by default; single tap center toggles. Left: Scores/Bookmarks/Setlists. Right: search, audio, tools. Center: title + Display Options + item menu. Double-tap center = Display Options. Bottom seek bar with bookmark names. Menus = overlay sidebar (dims page) or tiled sidebar [1][6] | Scores, Bookmarks (page ranges, treated like scores), Setlists (folders, placeholders, shuffle, manual order), rich metadata, libraries [7] | App dark mode **dims** pages, does not invert; Sepia effect; invert via iOS Smart Invert [8][9] | Audio panel docked **bottom**, shows/hides with chrome, pill handle; 25-200% speed, ±1 oct pitch, A-B loop by dragging handles, per-score settings [10] | Piano keyboard tool, metronome, pitch pipe, record; no score-aware playback (PDF) |
| **MobileSheets** | Single (horizontal), Two Pages (landscape only), Half Page, Vertical Scrolling (side tap scrolls next section; optional half-step); landscape single = two enlarged halves; per-song display mode; fill-width/screen scaling; auto crop margins; multi-tablet "book mode" [11][12] | Side taps, pedals, link points, auto-scroll | Center tap = song overlay, icon row at bottom of overlay [11] | Songs, setlists, collections, rich metadata | Night mode = **pure pixel invert** (colored chords flip hue) [13] | Audio/MIDI per song (n/c detail) | - |
| **Newzik** | PDF: double page landscape (1-page advance), half-page portrait w/ adjustable split, Best Fit; MusicXML: **reflow**: pinch sets staff size, systems re-wrap to width [14][15][16] | Pedals; MusicXML pedal = **one system per press**; face gestures (Premium); audio/video-synced turns [15][16] | Large toolbars criticised for covering score [16] | Setlists w/ folders, Projects = shared setlists; annotation layers private/public/shared [16] | No full dark; Sepia; Lyrics&Chords black bg [16] | LiveScore: **green current-measure highlight + playback line** (toggled separately), mixer volume/mute/solo, show/hide staves, tempo, loop, transpose; Web: inspector side panel w/ MIDI mixer [14][17] | Part extraction from score; Smart Annotations auto-highlight elements |
| **Piascore** | Single, half-page (portrait), auto vertical scroll w/ speed [18] | Taps, pedals, **face gestures** (wink L/R, ~0.5 s hold, ~2 s lockout, icon greys), paired iPhone "Air" turn; gaze turning beta [18][19] | Smile icon top-right toggles gestures | Basic; IMSLP integration [20] | n/c | none | Users report mouth gesture fails while singing [18] |
| **Henle Library** | Single/two-page; adjustable margins & staff spacing; background colour choice; **no half-page turn** (requested) [21] | Swipe/tap, pedals, jump by movement or 10-bar steps, **one-tap jump back to repeat start**, two-finger tap = repeat in Performance mode; no turn animation [21] | Center tap = menu mode; fingering icon top-right [21] | Purchased editions, playlists; score<->part switch | Background colour only | Metronome, record/playback | Toggle editorial layers (fingerings) over clean score |
| **nkoda** | Score View (single page) + Navigation View (scrolling overview, bookmarks, repeat marks) [22] | Corner taps (top/bottom R = next, L = prev), pedals [22] | "Dock" tool layout [23] | Subscription catalogue + own PDFs | n/c | Metronome | Review: sluggish, poor rendering [20] |
| **Musicnotes** | Page view; note highlight in play | taps, pedals | n/c | Set lists, folders (Pro sync) [24] | n/c | Interactive titles only: **audio mixer w/ vocal/instrument muting**, tempo, loop, transpose, notes light up [24][25] | Vocal mute = "without me" for singers |
| **MuseScore app** | Score playback, auto-scroll following playback [26] | n/c | n/c | Catalogue-driven | n/c | Pro: **mixer mute/solo/hide instrument**, metronome + count-in, transpose, piano [26] | Hide-others-but-hear = read only your staff |
| **StaffPad Reader** | Reflowed parts; max bars/line setting; "smart" read-ahead page turns without scroll [27] | auto in sync | n/c | n/c | **Dark mode** (performance) [27] | Progress-bar playhead; 3 listen modes: with orchestra / whole score / **your part isolated**; follow-playback toggle [27] | Listen modes map 1:1 to Cantor presets |
| **Tomplay** | Page view + **vertical continuous scroll** (2026); auto-scroll with cursor [28] | Auto; "smart page turn" listens (piano); pedals | "Audio" button **top-left** opens track list (mute/volume per track); Tempo menu w/ practice modes (Wait Mode) [28][29] | Catalogue | n/c | Track mute/volume, tempo, loop N measures, metronome; reviewers want full mute/solo mixer [29] | - |
| **SmartMusic** | Score w/ cursor modes beat / flow / off; measure highlight toggle; zoom [30] | auto | Tabs across top (Tracks, Display...); classic: pinned control strip (tempo, bar:beat, My Part/Accomp toggles) [30][31] | Assignments | n/c | Tracks tab: volume accomp / **my part** / metronome; loop [30] | "My Part" toggle next to tempo |
| **ChoirMate / Choir Player / cori** | Sheet + tracks | n/c | n/c | Director-managed | n/c | Track mixer: bring own voice forward, solo, mute & sing with others; SATB + backing, record own part [32] | Exactly Cantor's preset model |

## 2. Cross-app patterns

- **Chrome hidden by default, single tap center toggles** (forScore, MobileSheets, Henle). Side taps = turn. Double-tap center = display options (forScore).
- **Tap zones**: left ~1/3 back, right ~1/3 forward, center toggles chrome. nkoda uses corners. Avoid turning on accidental taps: Performance mode (Henle, Newzik) locks out editing.
- **Orientation drives mode**: portrait = single + half-page; landscape = two-up or width-fit vertical scroll. Users choose per score (MobileSheets per-song, forScore per-score).
- **Half-page turn** is a must-have (Henle gets complaints for lacking it). Adjustable split point (forScore, Newzik).
- **Reflow** exists only where source is symbolic (Newzik MusicXML, StaffPad Reader). Pedal advances **one system** in reflow. Cantor has MusicXML + Verovio, so reflow is cheap and a real differentiator vs PDF readers.
- **Playback follows with a measure highlight + playhead line** (Newzik green measure, StaffPad progress playhead, SmartMusic beat/flow cursor). Users want highlight and cursor toggled separately.
- **Transport/audio lives at the bottom**, appears with chrome (forScore). Mixer/track list via a top-corner button popover (Tomplay "Audio" top-left) or an inspector side panel (Newzik Web).
- **Rehearsal presets beat raw faders**: SmartMusic My Part toggle, StaffPad 3 listen modes, ChoirMate "bring own voice forward / mute & sing with others", Musicnotes vocal mute.
- **Dark mode**: most do not restyle the score. Options seen: dim page (forScore), sepia (forScore, Newzik), naive pixel invert (MobileSheets, iOS Smart Invert), true dark render (StaffPad, symbolic). Naive invert looks grey and muddy on scans [13]. Cantor renders SVG, so it can recolour cleanly.
- **Navigation**: bottom seek bar w/ bookmark labels (forScore), jump by movement/10 bars + one-tap repeat-back (Henle), Links for repeats (forScore), Navigation overview (nkoda).
- **Annotation entry**: tap-hold on page or Apple Pencil touch auto-enters; draggable toolbar; layers (forScore, Newzik).
- **Hands-free turns**: pedals universal; face gestures (Piascore, Newzik, forScore) unreliable for singers (mouth moves). Playback-driven auto-turn is the natural fit for Cantor.

## 3. Recommendations for Cantor

### 3.1 Named display modes

| Mode | What | When | Notes |
|---|---|---|---|
| **Page** (single) | 1 Verovio page fits the screen | Portrait default; mirrors printed choir copy (page/system numbers match conductor) | Turn = whole page. Follow mode turns after the last system ends |
| **Half-page turn** | During the bottom half, the top half already shows the next page; a split line marks the seam | Portrait while playing/following; essential for singers holding a folder posture | Auto-advance the top half once the cursor passes the split; split = system boundary (not px), so no system is cut |
| **Two-up** | 2 pages side by side, advance 1 (rolling) or 2 | Landscape default for "print-faithful" | Setting: advance 1 vs 2 (forScore pattern). Small staff on 8.3" mini, so offer Reflow instead |
| **Continuous scroll (vertical)** | Pages stacked or a single tall layout; follow keeps the cursor system at ~1/3 from top | Landscape width-fit; rehearsing with playback | Smooth scroll, not jump; manual drag pauses follow for N s then resumes (or shows a "Resume follow" chip) |
| **Reflow** (symbolic only) | Verovio re-layout to screen width/height at user staff size (pinch, debounced) | Small iPad, bad eyesight, landscape; best default for **My part** focus | Pedal/tap = 1 system or 1 screen. Option: hide other staves (show only my voice + piano reduction) or condense SATB. Breaks page-number fidelity, so show measure numbers every system |
| **Horizontal continuous** (later) | One long system line, scrolls left | Note-by-note learning, karaoke-style with lyrics | Low priority; few readers use it |

Rule: default mode chosen by orientation, overridable per score (persist). Rotation keeps the current measure on screen.

### 3.2 Chrome behaviour

- Immersive by default during playback: chrome auto-hides 3 s after play or last interaction; never auto-hides while paused in mixer.
- Tap zones: left 30% = back, right 30% = forward (page / half / system per mode), center = toggle chrome. Double-tap center = Display sheet. **Tap on a measure while chrome visible = seek** (already in demo); in immersive, keep center tap for chrome only so a turn tap never seeks by accident. Alternative: seek = long-press.
- Top bar (overlay, translucent): left = Library/back, Setlist prev/next; center = title + voice/choir chip; right = Display (Aa), Mixer, Tools (metronome/pitch), More.
- Bottom bar = transport: play/pause, bar:beat readout, scrubber with rehearsal-mark/section ticks (forScore-style labels), tempo % chip, loop A-B toggle, follow toggle.
- Thin persistent progress line plus a measure chip may stay visible in immersive (StaffPad-style), 2-4 px, so users keep orientation.
- Performance lock (later): disables seek/annotation taps, only turns.

### 3.3 Mixer placement

- **Presets first, faders second**: segmented control `All | My part | Without me | Balance` + "My voice: S/A/T/B" picker always visible in the mixer header. Faders/mute/solo per voice below (collapsed on small screens).
- **Portrait**: bottom sheet docked above transport. Detents: *peek* (presets + my-voice picker, ~1 row, can stay open while reading), *half* (all voice strips horizontal), dismiss. Score area shrinks rather than being covered, so the cursor system stays visible (scroll offset compensates).
- **Landscape**: right-side panel (~280-320 pt), tiled not overlay, so the score reflows/fits the remaining width (forScore tiled sidebar pattern). Collapsible to a slim vertical strip of per-voice M/S buttons.
- Also: quick mini-mixer popover from the top-bar Mixer button when the panel is closed (Tomplay pattern) for one-off mutes.
- Tempo + loop live in transport, not in the mixer.

### 3.4 Score highlighting for choir

- Current measure: subtle tinted band across the system (Newzik green-ish), plus optional playhead line (separate toggles; SmartMusic beat/flow options later).
- **My part highlight**: tint my staff background or colour my notes/lyrics (Verovio SVG class per staff/layer); dim other staves to ~60% opacity (option). Muted voices drawn greyer to mirror audio state.
- Lyrics: optional enlarged lyric size for my staff in Reflow.
- Rehearsal marks/sections as scrubber ticks + "Jump to" list; one-tap "back to repeat/section start" (Henle).

### 3.5 Dark-mode paper treatment

Render-time recolouring of the SVG (CSS vars on `fill/stroke`). No bitmap invert.
- **Paper** (default light): off-white `#FBF8F1`, ink near-black.
- **Sepia**: warm paper `#F3E9D2`, ink `#3B2F23`. Good for long rehearsals.
- **Dim**: light paper with brightness reduced ~70%, dark chrome (forScore approach, keeps "music looks like music").
- **Night**: true dark: bg `#121212`-`#1A1A1A`, ink `#D8D8D8` (not pure white, less glare), highlight colours re-tuned for dark (desaturated). Annotations recoloured via same vars.
- Chrome follows system appearance. Paper theme is separate, with "Auto: Night when system dark" option.

### 3.6 Prioritised list

**MVP**
1. Page mode (portrait) + Continuous scroll (landscape width-fit), orientation defaults, keep measure on rotate.
2. Follow page turn tuned per mode (look-ahead: turn when cursor enters last system, half-page in portrait).
3. Tap zones L/R/center; chrome auto-hide in playback; double-tap = Display sheet.
4. Bottom transport (play, scrubber with section ticks, tempo %, follow toggle).
5. Mixer: presets + my-voice picker; portrait bottom sheet (peek/half), landscape tiled right panel.
6. Current-measure highlight + my-staff tint.
7. Paper themes: Paper, Sepia, Night (SVG recolour).
8. Bluetooth pedal/keyboard: arrows, space = play/pause, PgUp/PgDn.

**Next**
9. Half-page turn (system-aligned split).
10. Reflow mode with pinch staff size, show-only-my-voice option.
11. Two-up landscape (advance 1/2).
12. A-B loop on scrubber + count-in click; pitch pipe/starting-note button (choir must-have: "give me my note").
13. Rehearsal-mark jump list, back-to-section-start.
14. Library: setlists (concert programmes) with prev/next in score view; metadata (composer, voicing, choir).

**Later**
15. Annotation (Pencil auto-enter, layers), persisted per measure (symbolic anchoring survives reflow).
16. Performance lock, AirPods head-turn, external display/Cue-style sync for section leaders.
17. Dim theme, horizontal continuous/karaoke lyrics, cursor style options (beat/flow).
18. Record self + compare (Choir Player pattern).

## Sources

1. forScore Basics: https://forscore.co/documentation/basics/
2. forScore two-up: https://forscore.co/kb/2-up/ ; https://forscore.co/two-up-view/
3. forScore 13.0 guide (landscape scrolling, crop): https://forscore.co/forScore13-0.pdf
4. forScore Page Turners & Shortcuts: https://forscore.co/documentation/page-turners/
5. forScore Links & Buttons: https://forscore.co/documentation/links-buttons/
6. forScore Menus: https://forscore.co/documentation/menus/
7. forScore Setlists: https://forscore.co/documentation/setlists/
8. forScore design (dark mode dims pages): https://forscore.co/about-design/
9. forScore KB inverting screen: https://forscore.co/kb/inverting-screen/
10. forScore Audio: https://forscore.co/documentation/audio/
11. MobileSheets display features: https://zubersoft.com/mobilesheets/features/display/
12. MobileSheets user guide: https://www.zubersoft.com/mobilesheets/MobileSheetsPro.pdf
13. MobileSheets forum, night mode invert: https://www.zubersoft.com/mobilesheets/forum/thread-11319.html ; Dark Score: https://darkscore.app/
14. Scoring Notes, Newzik Maestria review: https://www.scoringnotes.com/reviews/newzik-maestria-review/
15. Scoring Notes, best iPad score reader: https://www.scoringnotes.com/reviews/the-best-ipad-score-reader-for-most-people/
16. sheetmusicapps Newzik review: https://www.sheetmusicapps.com/reviews/newzik/
17. Newzik Web score view: https://support.newzik.com/en/support/solutions/articles/77000275101-the-score-view-on-newzik-web ; LiveScores: https://support.newzik.com/en/support/solutions/articles/77000495892-what-are-livescores-
18. Piascore App Store: https://apps.apple.com/au/app/piascore-smart-music-score/id406141702 ; review: https://www.sheetmusicapps.com/reviews/piascore/
19. Piascore gesture help: https://help.piascore.com/hc/en-us/articles/4407316321037-How-to-turn-a-page-by-gesture-wink
20. Scoring Notes (as 15)
21. Henle Library features: https://henle-library.com/en/features/ ; App Store: https://apps.apple.com/us/app/henle-library/id1021283948
22. Scoring Notes nkoda review: https://www.scoringnotes.com/reviews/nkoda-review/
23. nkoda App Store: https://apps.apple.com/us/app/nkoda-the-sheet-music-library/id1327811418
24. Musicnotes iOS: https://www.musicnotes.com/apps/ios/ ; blog: https://www.musicnotes.com/blog/musicnotes-player-app/
25. sheetmusicapps Musicnotes review: https://www.sheetmusicapps.com/reviews/musicnotes/
26. sheetmusicapps MuseScore review: https://www.sheetmusicapps.com/reviews/musescore/
27. StaffPad Reader: https://apps.apple.com/us/app/staffpad-reader/id1258751006 ; https://www.scoringnotes.com/reviews/staffpad-for-ipad/
28. Tomplay how it works: https://tomplay.com/how-it-works ; sheetmusicapps review: https://www.sheetmusicapps.com/reviews/tomplay/
29. Tomplay audio tracks help: https://tomplay.zendesk.com/hc/en-us/articles/7732871683996-Choose-audio-tracks-play-along ; PedaPlus review: https://www.pedaplus.com/review-tomplay-interactive-sheet-music/
30. New SmartMusic guide: https://4.files.edl.io/dfe6/06/21/18/182452-6b0bd1f6-2d40-40fe-ada0-b6750bbbcc1e.pdf
31. SmartMusic My Part: https://usermanuals.smartmusic.com/onlinehelp/mobile/Content/hear_your_part.htm
32. ChoirMate: https://apps.apple.com/us/app/choirmate/id1495545798 ; Choir Player: https://apps.apple.com/us/app/choir-player-learn-practice/id1139783014 ; cori: https://cori.music/en

Gaps: Sing Sharp is a solo AI vocal coach with no voice mixer, so it was dropped. Notion reader, Carus Choir Coach, and nkoda/Musicnotes dark modes are unverified. The Newzik iPad mixer placement and MuseScore mobile mixer placement are also unverified.
