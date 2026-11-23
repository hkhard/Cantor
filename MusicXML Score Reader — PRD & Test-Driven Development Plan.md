# MusicXML Score Reader — PRD & Test-Driven Development Plan

Nov 21, 2026 · @Hans Hård

## Summary

We build a forScore-class sheet music reader that also understands the music: it opens MusicXML, plays every part automatically, lets the musician solo, mute or rebalance any part, and can optionally sing the lyrics with a bring-your-own-key voice engine. Working name: **Cantor**.

- **Platforms:** iPadOS and iOS first (where forScore users are), then macOS, Windows, Android and a web/PWA build from the same code base.
- **Core difference from forScore:** forScore reads PDFs and cannot interpret the notes in them; Cantor reads the notes themselves (MusicXML/MXL), so playback, part balance, follow-along cursor and sung lyrics are exact.
- **PDF stays first-class:** PDFs are read, annotated and organised like in forScore; playback for PDFs comes later via optical music recognition (OMR) to MusicXML.
- **Voice:** a pluggable `VocalProvider` with two families — score-accurate singing voice synthesis (open models on Hugging Face) and ElevenLabs (user's own API key). Section 5 explains why only the first can follow the notated melody exactly today.
- **Method:** test-driven throughout. Every requirement below has an ID and acceptance criteria that become automated tests before the code is written.

Terminology: the request says "every system". In engraving a *system* is one line of staves across the page; this PRD reads it as **every part / staff / voice** (for example Soprano, Alto, Tenor, Bass, Piano RH/LH). Confirmed on 2026-11-21.

## forScore benchmark

forScore (v15.2, iOS/iPadOS/macOS/visionOS) is the parity target for reading, annotating and performing; Cantor matches its core in v1 and wins on note-aware playback. forScore itself cannot interpret the notes in a PDF — even its Reflow only detects where systems sit on the page ([forScore KB](https://forscore.co/author/forscoreapp/page/27/)).

| Area | forScore today | Cantor v1 | Cantor later |
| --- | --- | --- | --- |
| Library | PDF import from Files, cloud services, Musicnotes; metadata, libraries, ratings, difficulty, search ([App Store](https://apps.apple.com/app/id363738376)) | PDF + MusicXML/MXL import, metadata, tags, search, ratings | MEI, MIDI import; OMR of PDF to MusicXML |
| Setlists | Unlimited setlists, placeholders, sharing with non-forScore users ([About](https://forscore.co/about-power)) | Setlists, placeholders, share as file bundle | Live shared setlists |
| Display | Single page, two-up, half-page turns, partial turns in multi-page view, Best Fit / Zoomed modes ([KB](https://forscore.co/kb), [15.2](https://forscore.co/?p=2208)) | Same for PDF; MusicXML re-engraved to fit any screen | Continuous scroll |
| Reflow | Detects systems in a PDF and lays them end to end like a teleprompter ([Design](https://forscore.co/about-design)) | True reflow: MusicXML re-laid out to screen width | PDF reflow via OMR |
| Annotation | Apple Pencil always-on drawing, layers, undo/redo, shapes (slurs, crescendos), stamps, text, snapshots, embedded PDF annotations ([About](https://forscore.co/about-power)) | Pencil/stylus ink, layers, stamps, text, undo; anchored to measures in MusicXML | Shape recognition |
| Page turning | Taps, Bluetooth pedals, Face Gestures, Wave ring, autoturn tied to audio ([13](https://forscore.co/thirteen/), [15.1](https://forscore.co/?p=2187)) | Taps, keyboard/HID pedals, **auto-turn from playback position** | Face gestures, score following by microphone |
| Audio | Backing tracks, 25–200% speed without pitch change, pitch shift, page turns on the track timeline ([forScore 10](https://forscore.co/?p=843)) | Backing tracks with time-stretch | Track-to-score alignment |
| Playback of notes | MIDI playback of attached files; transposition and playback listed as limited ([KB](https://forscore.co/kb/category/03-before-you-buy/)) | **Synthesised playback of every part, per-part mixer, solo/mute, transpose, tempo, loop** | — |
| Practice tools | Metronome, pitch pipe, tempo per score, Dashboard goals ([Features](https://forscore.co/features-of-the-year)) | Metronome, pitch pipe, count-in, loop A–B | Practice stats |
| Ensemble | Cue: leader sends page turns to connected devices ([News](https://forscore.co/category/news)) | — | Leader/follower sync |
| Singing | — | **Sung lyrics per part (BYO key)** | More voices and languages |
| Sync | Device sync via logs ([KB](https://forscore.co/kb/category/00-important-updates/)) | Folder-based sync (iCloud / OneDrive / Drive) | Own sync service |

forScore's own .4SC (score), .4SS (setlist) and .4SB (backup) files must be imported in v1 so users can move their library with annotations intact (LIB-07, LIB-08). The format is undocumented, so it is reverse-engineered from real example files in M0.

## Users, goals and success metrics

The primary user is a choir or ensemble singer learning a part; the secondary users are instrumentalists who perform from a tablet and directors who prepare rehearsal tracks.

| Persona | Job to be done | Must-have |
| --- | --- | --- |
| Choir singer | Learn my alto line at home, hearing it loud over the others | Solo/boost one part, loop a passage, slow tempo, sung lyrics |
| Instrumentalist | Perform from the tablet without fumbling page turns | Fast PDF reading, pedal turns, auto-turn, annotations |
| Director / teacher | Make practice tracks for each section of the choir | Per-part mix presets, export audio per part |
| Accompanist | Rehearse with a sung melody when the singers are absent | Sung parts with piano muted |

**Goals (v1)**

1. Any valid MusicXML 3.1/4.0 score renders and plays every part with correct pitch, rhythm, repeats and tempo.
2. Each part has its own volume, pan, mute and solo, plus one-tap "my part loud, others quiet" presets.
3. Reading, annotating and setlist use of PDFs feels as fast as forScore on an iPad.
4. Lyrics can be sung on a part with the user's own ElevenLabs or Hugging Face credentials, with no Cantor-paid inference — English at launch, Swedish next.

**Non-goals (v1)**

- A notation editor (beyond transpose, part naming and lyric fixes).
- Converting PDF to MusicXML (OMR) — planned for v2.
- Real-time score following by microphone.
- Hosting a paid voice service; voice is bring-your-own-key only.

**Success metrics**

| Metric | Target |
| --- | --- |
| Conformance: share of the test corpus that imports, engraves and plays without error | ≥ 98% |
| Playback timing drift against the score timemap after 10 minutes | < 5 ms |
| Cold open of a 20-page PDF on iPad (A14 or newer) | < 1 s to first page |
| Page turn latency (pedal press to new page drawn) | < 50 ms |
| Sung-lyrics alignment: syllable onsets within one sixteenth note at the score tempo | ≥ 95% of syllables |
| Crash-free sessions | ≥ 99.8% |

## Functional requirements

Each requirement has an ID, a priority (P0 = v1 must, P1 = v1 should, P2 = later) and acceptance criteria written so they can be turned directly into tests. Test IDs in section 8 reuse these IDs.

### Import and library (LIB)

| ID | Pri | Requirement | Acceptance criteria |
| --- | --- | --- | --- |
| LIB-01 | P0 | Import MusicXML (.musicxml, .xml) and compressed MXL | Every file in the conformance corpus parses; invalid files produce a readable error, never a crash |
| LIB-02 | P0 | Import PDF | 500-page PDF imports in < 3 s; thumbnails generated lazily |
| LIB-03 | P0 | Metadata: title, composer, arranger, key, tempo, genre, tags, rating, difficulty | Edited metadata survives app restart and export/import round trip |
| LIB-04 | P0 | Search across metadata and part names | 10,000-item library returns results in < 100 ms |
| LIB-05 | P1 | Link a PDF and a MusicXML of the same piece as one score | Playback cursor shows on the PDF when a measure map exists (manual or OMR) |
| LIB-06 | P2 | OMR: PDF → MusicXML via Audiveris (desktop/server) | Output opens in Cantor and plays; user can correct and re-save |
| LIB-07 | P0 | Import forScore .4SC scores: PDF, metadata, annotations, bookmarks, links, audio/MIDI attachments | Every example file imports; page count, metadata and annotation positions match screenshots from forScore within 2 pt; unknown fields kept, never dropped silently |
| LIB-08 | P0 | Import forScore .4SS setlists and .4SB backups | Setlist order, placeholders and item start pages match; a full backup imports into a library with the same score count and setlists |
| LIB-09 | P2 | Export to .4SC / .4SS for users going back to forScore | Round trip Cantor → forScore → Cantor keeps annotations |

### Rendering and display (VIEW)

| ID | Pri | Requirement | Acceptance criteria |
| --- | --- | --- | --- |
| VIEW-01 | P0 | Engrave MusicXML to SVG pages that fit the screen | Golden SVG/PNG snapshots match the reference within tolerance |
| VIEW-02 | P0 | Page modes: single, two-up, half-page turn, continuous scroll | Each mode reachable in one tap; position kept on rotation |
| VIEW-03 | P0 | True reflow of MusicXML on narrow screens | Phone width shows ≥ 2 measures per system at default zoom |
| VIEW-04 | P1 | Show / hide individual parts (e.g. only Alto + Piano) | Hidden parts disappear from layout but can still play |
| VIEW-05 | P1 | Transpose display and playback by semitones; concert / transposed pitch toggle | Transposing +2 then −2 gives a model equal to the original |
| VIEW-06 | P1 | Dark and sepia themes | Contrast ≥ 4.5:1 for notation and text |
| VIEW-07 | P1 | Landscape on a tablet: the PDF page fills the screen width, so a whole system is shown at least as wide as the screen; scroll down for the rest of the page | In landscape the page is as wide as the reader pane and the playing system is fully visible; portrait unchanged |

### Playback and mixer (PLAY)

| ID | Pri | Requirement | Acceptance criteria |
| --- | --- | --- | --- |
| PLAY-01 | P0 | Play all parts automatically from any measure | Event timeline matches the reference MIDI of the corpus (pitch, onset, duration) |
| PLAY-02 | P0 | Honour repeats, voltas, D.C./D.S./Coda/Fine, ties, tuplets, grace notes, tempo and meter changes | Unfolded measure order equals expected order for every navigation fixture |
| PLAY-03 | P0 | Per-part channel: volume (−∞ to +6 dB), pan, mute, solo | Solo isolates one part; mute+solo rules follow DAW conventions (tests per truth table) |
| PLAY-04 | P0 | Level presets: "My part" (selected part 0 dB, others −12 dB), "Without me" (selected muted), "Balanced" | Applying a preset sets exactly the expected gains; user can save custom presets per score |
| PLAY-05 | P0 | Per-voice control inside one staff (e.g. S and A sharing a staff) | Voice 1 and voice 2 of a staff appear as separate mixer channels |
| PLAY-06 | P0 | Tempo 25–200% without pitch change; count-in; metronome click | Tempo change mid-playback applies on the next beat, no glitch |
| PLAY-07 | P0 | Loop A–B by measure range | Loop wraps sample-accurately (≤ 1 audio buffer jitter) |
| PLAY-08 | P0 | Follow-along cursor and auto page turn | Cursor highlights the sounding note; page turns before the last system ends |
| PLAY-09 | P0 | Instrument per part from General MIDI names in MusicXML, overridable | `<midi-program>` respected; fallback by part name ("Soprano" → choir aahs) |
| PLAY-10 | P1 | Dynamics and articulations affect velocity and length | pp < mf < ff velocities; staccato shortens to 50% |
| PLAY-11 | P1 | Export mix to WAV/M4A, one file per preset or per part | Exported file length equals score length at the chosen tempo ± 10 ms |
| PLAY-12 | P1 | Background audio and lock-screen / media key controls | Playback continues with screen locked on iOS and Android |

### Sung lyrics (VOX)

| ID | Pri | Requirement | Acceptance criteria |
| --- | --- | --- | --- |
| VOX-01 | P0 | Extract lyrics per part and verse, with syllables, hyphens, melismas and extenders | Lyric fixture set produces the expected syllable-to-note list |
| VOX-02 | P0 | User adds own ElevenLabs and/or Hugging Face key in Settings | Keys stored only in OS keychain; never logged; deletable; validated with a test call |
| VOX-03 | P0 | "Sing this part" renders a vocal track for a chosen part and verse | Rendered track is placed on the timeline and appears as a mixer channel |
| VOX-04 | P0 | Score-accurate mode follows notated pitch and rhythm | Pitch error < 50 cents median; onsets within one sixteenth note (metric in section 3) |
| VOX-05 | P1 | ElevenLabs "performance" mode: generated song from the score's lyrics, style and length | Clearly labelled as not following the notated melody; aligned by word timestamps |
| VOX-06 | P0 | Cost and time preview before rendering | Shows estimated seconds of audio / credits; requires confirmation |
| VOX-07 | P0 | Cache renders per (part, verse, tempo, transpose, voice, provider) | Re-opening a score reuses the cached track without a new API call |
| VOX-08 | P1 | Tempo and transpose changes re-render only the affected phrases | Changing tempo re-renders; muting does not |
| VOX-09 | P1 | Voice choice: provider voice list or a short reference clip the user owns | Consent checkbox for reference clips; clip never leaves the chosen provider |

### Annotation (ANN)

| ID | Pri | Requirement | Acceptance criteria |
| --- | --- | --- | --- |
| ANN-01 | P0 | Pencil/stylus ink with pressure, eraser, highlighter, undo/redo | Ink latency < 20 ms on iPad Pro; 100-step undo |
| ANN-02 | P0 | Layers that can be shown or hidden | Layer visibility persists per score |
| ANN-03 | P0 | Text, stamps (fingerings, breath marks, dynamics), shapes | Stamps scale with zoom |
| ANN-04 | P0 | Annotations on MusicXML anchored to measure + offset so they survive re-layout | After reflow, each annotation stays on its measure (fixture test) |
| ANN-05 | P1 | Export annotated PDF | Annotations visible in Preview / Acrobat |

### Setlists and performance (SET / PERF)

| ID | Pri | Requirement | Acceptance criteria |
| --- | --- | --- | --- |
| SET-01 | P0 | Setlists with ordering, placeholders and per-item start page | Drag to reorder; placeholder shows in performance mode |
| SET-02 | P1 | Share setlist as a bundle (scores + annotations + mix presets) | Bundle imports on another device with identical content hash |
| SET-03 | P1 | Instructions per score and per setlist: free text with paragraphs, bold, links and bar references ("t. 12", "m. 12") that jump to the bar; plus "who sings what" hints (member or group to part) per score | Saved in the library store, synced through folder sync, in backups; shown and editable in the reader and on the setlist page; a bar reference seeks to that bar; a member who picks their name gets their part marked |
| SET-04 | P1 | Account-free set sharing: the `.cantorset` carries the setlist, its instructions, every score's files, the part hints and exported practice tracks (one M4A per part); older bundles stay readable | Round trip keeps instructions, hints and tracks with the same content hash; a version 1 bundle imports; opening a received set shows its instructions and offers "practise my part" without a re-render |
| SET-05 | P2 | A note per setlist item ("start at bar 9", "skip verse 2") | Saved with the setlist, shown in the list and in the reader, travels in the bundle |
| PERF-01 | P0 | Performance mode: locks editing, large turn zones | No accidental annotation in performance mode |
| PERF-02 | P0 | Page turners: Bluetooth HID pedals (AirTurn, PageFlip), keyboard arrows | Pedal press maps to configurable action |
| PERF-03 | P1 | Metronome and pitch pipe | Metronome jitter < 1 ms against the audio clock |
| PERF-04 | P1 | Backing tracks with time-stretch and page turns on the track timeline | Turn markers fire within 50 ms |
| PERF-05 | P1 | Turn by swipe: a left/right swipe turns the page with a quick page-fold animation (an option, on by default) | A swipe turns one page; a vertical drag or a slow drag does not; no animation with reduced motion |
| PERF-06 | P1 | Bar by bar: an option where tapping the right or left side moves the cursor one bar forward or back, so the whole score can be tapped through; the page follows the bar | Tapping right N times lands on bar N+1; tapping left goes back; works on the PDF and the notation |

### Sync and data (DATA)

| ID | Pri | Requirement | Acceptance criteria |
| --- | --- | --- | --- |
| DATA-01 | P0 | Local-first library; works fully offline except voice rendering | All P0 features except VOX-03 pass with network disabled |
| DATA-02 | P1 | Sync through a user-chosen folder (iCloud Drive, OneDrive, Google Drive) | Two devices converge after concurrent edits (CRDT test) |
| DATA-03 | P0 | Full backup / restore file | Restore yields identical library hash |

### Band: chord symbols and instrumentalists (BAND)

Groundwork for the future Cantor Band edition, built in the free client. Count-in and click already exist (PLAY-06, PERF-03). None of these are P0, so the v1 gate does not change.

| ID | Pri | Requirement | Acceptance criteria |
| --- | --- | --- | --- |
| BAND-01 | P1 | Read chord symbols (MusicXML `<harmony>`: root, alter, kind, bass, degrees, offset) into the model with their timing, keep them when writing | The lead-sheet fixture yields the expected list of (measure, tick offset, symbol); `parse(write(s)) == s` with chords |
| BAND-02 | P1 | Chord symbols move with every transposition (VIEW-05 semitones, concert pitch, instrument key), spelled for the new key: flats in flat keys (Bb, not A#), sharps in sharp keys | C7 in C up 1 is Db7 in Db; F up 2 gives G; G in G down 2 gives F; +n then −n gives the same chords; the transposed MusicXML text equals the transposed model |
| BAND-03 | P1 | Instrument key per player: C, B♭, E♭ or F. The notation is shown written for that instrument (B♭: a major second up; E♭: a major sixth up; F: a perfect fifth up), while playback stays at concert pitch | Choosing B♭ shows concert C as D and C7 as D7; the timemap's pitches are unchanged; the choice is saved per score and restored after a restart (UI scenario) |
| BAND-04 | P2 | Lead-sheet view: melody, chord symbols and lyrics only, one staff, other parts hidden | A SATB score with chords shows one staff with all chord symbols; switching back restores all parts |
| BAND-05 | P2 | Chord practice playback: a toggleable comping channel plays block chords from the chord symbols (piano, one chord per symbol, held until the next) | The comp notes at each chord's tick are exactly its chord tones (C7 = C E G B♭; slash bass in the bass); muting the channel silences it; scores without chords have no comp channel |

## Sung lyrics: ElevenLabs and Hugging Face

Singing the notated part is feasible today, but only with score-conditioned open models from Hugging Face; ElevenLabs can sing the lyrics in a generated song, not the written melody. So Hugging Face is the default engine for "sing my part", and ElevenLabs is an optional "performance" mode.

**Why ElevenLabs cannot follow the score.** The Eleven Music API composes from a prompt or a composition plan. A plan is up to 30 chunks of 3–120 s (10 min total); each chunk carries `text` (section name and lyric lines), `duration_ms`, positive/negative style words and `context_adherence`. There is no input for pitches, note values or MIDI; melody can only be described in words ([composition plans](https://elevenlabs.io/docs/eleven-api/guides/how-to/music/composition-plans)). Word-level timestamps are available, which lets us line the result up with measures. `music_v1` is deprecated on 31 January 2027, so we target `music_v2_5`.

**Score-accurate engines on Hugging Face**

| Engine | Score input | License | Runs on | Fit for Cantor |
| --- | --- | --- | --- | --- |
| [SoulX-Singer](https://huggingface.co/Soul-AILab/SoulX-Singer) | MIDI notes (score) or F0 contour; zero-shot voice from a reference clip | Apache-2.0 | GPU server | Best default: multilingual, permissive license |
| [VocalRender](https://huggingface.co/pymaster/VocalRender) | Lyrics + MIDI pitch + note values + BPM; accepts MusicXML; 48 kHz output; needs a 2–8 s reference clip | Apache-2.0 | CUDA GPU, \~9.5 GB per checkpoint | Closest to MusicXML, but mainly Mandarin today |
| [OmniSinger](https://huggingface.co/mindvu/OmniSinger) | `word{pitch:duration}` per word, MIDI pitch and centiseconds | Check before use | 0.6B params: GPU, possibly Apple silicon | Simplest adapter; small enough to try on-device on Mac |
| [NNSVS](https://github.com/nnsvs/nnsvs) | MusicXML via the Sinsy toolchain | MIT (toolkit); voices vary | CPU/GPU | Fallback, offline, robotic but exact |

**Pipeline (provider-independent)**

1. `LyricExtractor` turns a part + verse into phrases: syllables, `syllabic` begin/middle/end, melismas (one syllable over several notes), ties and rests. Phrases break at rests ≥ one beat or at breath marks.
2. `TimingResolver` converts each note to seconds using the same unfolded timemap that drives playback, at the user's tempo and transpose.
3. A provider adapter serialises the phrase into its format (SoulX MIDI, VocalRender JSON, OmniSinger tags, or an ElevenLabs composition plan).
4. Renders run phrase by phrase, are cached by content hash (VOX-07), and are time-aligned to phrase onsets before entering the mixer as a normal channel.

**Where Hugging Face models run.** The user supplies a Hugging Face token and either (a) their own Inference Endpoint or Space URL for the model, or (b) a local server on their Mac/PC with a GPU. No current serverless Inference Provider hosts these models, so "paste a token and go" is not possible yet. Phones always call a remote endpoint.

**Hybrid to test in a spike.** Render a score-accurate guide vocal with SoulX-Singer, then pass it through ElevenLabs speech-to-speech to apply an ElevenLabs voice. Whether that preserves pitch on sung input is unverified; spike S3 measures it.

**Language risk.** The models are trained mostly on Chinese and English. English is the launch language and is well covered. Swedish follows as the second language; its quality is unknown and is tested in spike S2 with three Swedish choral pieces so we know early whether it needs fine-tuning or phoneme overrides.

**Consent and safety.** A reference clip must be the user's own voice or one they have rights to (checkbox + stored provenance). Cantor never uploads a clip to a provider other than the one the user chose.

## Architecture and tech stack

A single Rust core holds the music model, timing, audio and voice logic; a thin Tauri 2 shell gives one UI on iPadOS, iOS, macOS, Windows and Android, and the same core compiles to WASM for a web build.

&#91;embedded content: Cantor architecture · shell, Rust core, outside services\]

The unfolded timemap is the one source of time: the scheduler plays notes from it, the cursor and page turns follow it, and sung tracks are rendered against it and return to the mixer as ordinary channels.

| Layer | Choice | Why | Alternative considered |
| --- | --- | --- | --- |
| App shell | Tauri 2 + TypeScript (Svelte) | One code base for desktop and mobile, small binaries, Rust native plugins | Flutter (weaker web notation libs); native Swift + Kotlin (two UIs) |
| Engraving | Verovio (C++ → WASM) rendering MusicXML/MEI to SVG, with its MIDI/timemap output as a test oracle | Mature, reflows to any width, exposes note-to-time map | OpenSheetMusicDisplay (TS, VexFlow) as fallback |
| PDF | PDF.js on web/desktop, PDFKit on Apple via plugin | Fast, annotation-friendly | pdfium |
| Model and parsing | Rust crate `cantor-score` (quick-xml, zip for MXL) | Pure, fast, fully unit-testable | musicxml crates as reference |
| Audio engine | Rust: `cpal` output, `rustysynth` SoundFont synth, own sample-clock scheduler and mixer graph | Low latency, deterministic offline rendering for tests | FluidSynth (LGPL, C) |
| Time-stretch | Rubber Band or Signalsmith Stretch for backing tracks and sung tracks | High quality tempo change without pitch shift | SoundTouch |
| Sound | MuseScore General (MIT) bundled as SF2; user SoundFonts allowed | Good choir and piano patches, redistributable | GeneralUser GS |
| Storage | SQLite (library, setlists, mixes), files for scores, CRDT (Automerge) for annotations | Local-first, mergeable sync | Core Data/CloudKit (Apple only) |
| Secrets | OS keychain via Tauri stronghold/keyring plugin | Keys never in app storage | — |
| Voice | `VocalProvider` trait: `estimate()`, `render(phrase)`, `voices()`; adapters for SoulX-Singer, VocalRender, OmniSinger, NNSVS, ElevenLabs Music | Swap engines without touching playback | — |
| OMR (v2) | Audiveris (Java) as a desktop sidecar or server | Best open OMR to MusicXML | oemer |

**Repository layout**

```
cantor/
  crates/score      # MusicXML/MXL parse, model, transpose
  crates/timeline   # unfold repeats, tempo map, timemap
  crates/audio      # scheduler, synth, mixer, offline render
  crates/lyrics     # syllables, melismas, phrases
  crates/vocal      # VocalProvider trait + adapters, cache
  crates/library    # SQLite, setlists, sync, backup
  app/              # Tauri shell + Svelte UI
  fixtures/         # corpus + expected outputs
  tools/oracle      # Verovio-based reference generators
```

## Non-functional requirements

| ID | Area | Requirement |
| --- | --- | --- |
| NFR-01 | Audio latency | Output buffer ≤ 10 ms on iPad/Mac, ≤ 20 ms on Android/Windows; no dropouts at 64 parts on an iPad A14 |
| NFR-02 | Timing | Audio scheduler is sample-clock driven; UI cursor derived from the audio clock, never the reverse |
| NFR-03 | Performance | 60 fps scrolling and page turns; engraving of a 100-page score off the UI thread, first page < 1.5 s |
| NFR-04 | Memory | ≤ 400 MB with a 50 MB SoundFont and a 100-page score loaded on iPad |
| NFR-05 | Offline | Everything except voice rendering works offline (DATA-01) |
| NFR-06 | Security | API keys in Keychain / Android Keystore / Windows Credential Manager; TLS 1.2+; no keys in logs, crash reports or backups |
| NFR-07 | Privacy | No account required; no telemetry by default; opt-in crash reporting; GDPR-ready data export |
| NFR-08 | Accessibility | VoiceOver/TalkBack labels on all controls; Dynamic Type; WCAG 2.2 AA contrast; full keyboard control on desktop |
| NFR-09 | Licensing | Only SoundFonts and models with licences that allow redistribution or user-side download; licence of each listed in About |
| NFR-10 | Localisation | English and Swedish UI at launch |
| NFR-11 | Quality gates | ≥ 85% line coverage in core crates, 100% of P0 acceptance criteria automated, zero failing tests on main |
| NFR-13 | Robustness | No panic reaches the app: every command returns a readable error; damaged or unsupported files are refused with a plain message and leave nothing half-imported; library files are written atomically |
| NFR-12 | Cost control | Voice provider calls capped per render (default 10 min of audio) with a visible running total |

## Test-driven strategy

Every change starts with a failing test that names a requirement ID; code is written only to make it pass, then refactored with the suite green. Most logic lives in a pure Rust core, so roughly 80% of tests run headless in milliseconds without a device.

**Rules of the road**

1. Red → green → refactor. A PR without a new or changed failing-first test is rejected unless it is a pure refactor.
2. Test names carry the requirement: `play_02_dal_segno_al_coda_unfolds_in_order`.
3. Definition of Done for a requirement: all its acceptance criteria automated, passing on the CI matrix, documented in the traceability table.
4. Bugs get a regression fixture first (the smallest MusicXML that reproduces it), then the fix.

**Test layers**

| Layer | Share | Tools | What it proves |
| --- | --- | --- | --- |
| Unit (core) | \~60% | `cargo test`, `proptest` | Parser, model, unfolding, timing, mixer maths, lyric extraction |
| Golden / snapshot | \~15% | `insta` snapshots, image diff (pixelmatch) | Timemap JSON, MIDI event lists, engraved SVG/PNG, lyric phrase lists |
| Audio offline render | \~8% | Offline render to buffer, RMS / spectral checks | Solo/mute/gain really change the output; loops are seamless; no clipping |
| Contract (providers) | \~5% | Mock servers, recorded fixtures (VCR-style) | Each VocalProvider adapter sends valid requests and handles errors, limits and retries |
| Integration (UI + core) | \~8% | Vitest + Playwright (web shell), WebDriver for Tauri | Import → play → mix → annotate flows |
| Device / E2E | \~4% | XCUITest on iPad, Android instrumented tests, pedal HID simulation | Latency, page turn, Pencil ink, background audio |
| Live nightly | — | Real ElevenLabs / HF endpoints with test keys | Provider drift; pitch accuracy of sung output |

**Fixtures**

- *Conformance corpus:* the public MusicXML unofficial test suite (one file per feature: repeats, tuplets, lyrics, transposing instruments), plus MuseScore, Dorico, Sibelius and Finale exports of the same 10 pieces.
- *Choral set:* 30 public-domain SATB scores from CPDL, including Swedish texts, with hand-checked reference MIDI and syllable lists.
- *Stress set:* 100-page orchestral score, 64 parts, extreme tempo and meter changes.
- Each fixture has expected outputs checked in: `*.timemap.json`, `*.events.json`, `*.lyrics.json`, `*.svg`.

**Property-based tests (examples)**

- Transposing by n then −n returns the original model.
- Unfolded duration equals the sum of measure durations along the unfolded path.
- For any gain settings, solo of part p renders a buffer whose correlation with p's solo render is > 0.99.
- Lyric extraction never assigns a syllable to a rest, and every `syllabic=begin` is closed by an `end`.

**Measuring sung output (VOX-04)** — run pitch tracking (pYIN or CREPE) on the render, compare each note's median pitch and onset to the score timemap, and fail the nightly run if the median error exceeds 50 cents or onsets drift more than one sixteenth note.

**CI matrix** — Linux (core + web), macOS (core, Tauri desktop, iOS simulator), Windows (Tauri desktop), Android emulator; nightly runs on one real iPad and one Android tablet.

## Development plan

Seven milestones take Cantor from an empty repo to a public beta in about 30 weeks with 2–3 developers; each core milestone ends at a gate that is a set of tests, not a demo.

&#91;embedded content: Roadmap · 7 milestones, 5 gates, \~30 weeks\]

M3 starts in week 9 on PDF reading, which does not depend on playback; M5 starts in week 16 once the timemap and mixer are stable. Every task below is listed test-first: the test that must fail before the work begins.

| Milestone | Test written first | Then build | Gate to pass |
| --- | --- | --- | --- |
| **M0 Foundations + spikes** (wk 0–3) | CI runs an empty `cargo test` and Playwright smoke test on all five targets | Repo layout, CI matrix, fixture corpus with expected outputs generated by the Verovio oracle; spikes S1 Verovio WASM in Tauri on iPad, S2 singing quality in English, then Swedish (3 pieces each, 3 engines); S5 reverse-engineer the .4SC/.4SS/.4SB formats from the example files, S3 ElevenLabs speech-to-speech on a sung guide, S4 Pencil ink latency in WebView | Spike reports decide engine defaults and Q6 |
| **M1 Score core** (wk 3–7) | `lib_01_*` parse every corpus file; `play_02_*` unfolding order per navigation fixture; transpose round-trip property | `crates/score`, `crates/timeline`: MusicXML/MXL parser, normalised model, repeat/volta/D.S./Coda unfolding, tempo map, timemap | ≥ 98% corpus parses; timemaps equal oracle on 100% of navigation fixtures |
| **M2 Playback + mixer** (wk 7–11) | Offline render: event list equals reference MIDI; solo/mute truth table; loop seam test; preset gain test | `crates/audio`: sample-clock scheduler, SoundFont synth, per-part and per-voice channels, presets, tempo, count-in, metronome, A–B loop | All PLAY P0 criteria green; NFR-01 latency met on iPad and Mac |
| **M3 Reader UI** (wk 9–16) | Playwright: import → open → turn page → play → solo alto; golden PNG of engraved pages; pedal HID simulation | Library, metadata, search, forScore import (.4SC/.4SS/.4SB), setlists, Verovio view with reflow, PDF view, page modes, follow cursor, auto-turn, performance mode, pedals | LIB, VIEW, SET, PERF P0 green; 20-page PDF opens in < 1 s on iPad |
| **M4 Annotation + sync** (wk 16–20) | Annotation stays on its measure after reflow; CRDT merge of concurrent edits; backup restore hash | Ink, layers, stamps, text, measure anchoring, PDF export, folder sync, backup/restore | ANN, DATA P0 green |
| **M5 Sung lyrics** (wk 16–25) | Lyric fixtures → expected phrase lists; adapter contract tests against mock servers; key never appears in logs; cost preview test | `crates/lyrics`, `crates/vocal`: extractor, timing resolver, SoulX-Singer + OmniSinger + ElevenLabs adapters, cache, keychain settings UI, render queue | VOX P0 green; nightly pitch test median < 50 cents on the choral set |
| **M6 Beta + hardening** (wk 25–30) | Accessibility audit tests; localisation completeness test (sv, en); 2-hour soak test with no drift > 5 ms | Performance tuning, Swedish UI, onboarding, TestFlight / Play beta / Windows installer, licence page | All P0 + NFR green on the device matrix; crash-free ≥ 99.8% across beta users |

**First two weeks, concretely**

- [ ] Create repo with the crate layout from section 6 and CI for Linux, macOS, Windows, iOS simulator, Android emulator
- [ ] Collect 40 corpus files and generate `*.timemap.json` and `*.events.json` with the Verovio oracle
- [ ] Write the first failing test: `lib_01_parses_hello_world_musicxml`
- [ ] Run spike S1 (Verovio WASM in Tauri on iPad), S2 (English then Swedish singing on three engines) and S5 (forScore file format from the example files)
- [ ] Decide answers to Q3, Q5 and Q6

## Risks, open questions and sources

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Singing models handle Swedish (second language) poorly | Sung lyrics unusable for Swedish choirs | Spike S2 before M5; phoneme override per syllable; fall back to English or NNSVS |
| No serverless host for SVS models | Users must deploy an endpoint or run locally | Ship a one-click Space/Endpoint template and a local Docker server; document GPU cost |
| ElevenLabs cannot follow notated melody | Users expect "sing my alto line" from it | Label it Performance mode; default to score-accurate engine |
| WebView audio/ink latency on iPad | Fails NFR-01 / ANN-01 | Audio runs in native Rust, not WebAudio; Pencil ink via native overlay plugin if web pointer events miss 20 ms |
| MusicXML exporters disagree | Wrong playback or layout | Multi-exporter corpus; normalisation pass; bug fixtures |
| Verovio licence (LGPL-3.0) in App Store builds | Licence compliance | Dynamic linking on desktop; WASM module loaded separately on mobile; legal review in M0 |
| Model licences and voice cloning misuse | Legal and reputational | Licence allow-list; consent checkbox; no third-party voice cloning |
| Scope creep toward a notation editor | Delays v1 | Editing limited to transpose, part names, lyric fixes |
| forScore format is undocumented and may change between versions | Imports break or lose annotations | Versioned parsers with the example files as golden fixtures; keep unknown fields; fall back to importing the embedded PDF alone with a warning; check forScore's terms before shipping |

**Open questions**

- **Q1** Answered: "every system" means every part/staff/voice.
- **Q2** Answered: yes, import forScore's own formats (LIB-07, LIB-08); example files provided by the product owner.
- **Q3** Commercial model: one-time purchase, subscription, or open source?
- **Q4** Answered: English at launch, Swedish second.
- **Q5** Should the director persona be able to publish part tracks to a choir (needs a backend), or is file export enough for v1?
- **Q6** Is a native Swift iPad app acceptable as a later "pro" shell if Tauri cannot meet Pencil latency?

**Sources** (opened 2026-11-21)

- forScore: [About – Power](https://forscore.co/about-power), [About – Design](https://forscore.co/about-design), [Knowledge base](https://forscore.co/kb), [forScore 15](https://forscore.co/fifteen), [15.1](https://forscore.co/?p=2187), [15.2](https://forscore.co/?p=2208), [App Store listing](https://apps.apple.com/app/id363738376)
- ElevenLabs: [Composition plans](https://elevenlabs.io/docs/eleven-api/guides/how-to/music/composition-plans), [Eleven Music API](https://elevenlabs.io/music-api)
- Hugging Face / open SVS: [VocalRender](https://huggingface.co/pymaster/VocalRender), [SoulX-Singer paper](https://arxiv.org/abs/2602.07803v2), [OmniSinger](https://huggingface.co/mindvu/OmniSinger), [NNSVS paper](https://arxiv.org/pdf/2210.15987)
