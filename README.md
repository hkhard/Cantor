# ChoirBird

> Formerly Cantor. The code still uses `cantor` as its internal name
> (crates `cantor-*`, the repository, the Xcode project); users see
> ChoirBird. BandBird and ConductorBird are planned sister apps.

**Sheet music that plays every part.** ChoirBird is a choir practice app
and score reader for choir singers. It shows the original PDF, plays every voice with its own
instrument and follows along with a cursor, so you can rehearse your own
line. It works offline on the iPad.

- **Library:** import MusicXML, MXL, PDF and forScore files (`.4sc`, `.4ss`).
  - Metadata, search and setlists.
  - Drag to rearrange, iOS style.
  - "Läs från sidan" fills in the title and composer from the printed page,
    using on-device text recognition.
- **Reader:**
  - The original PDF, or re-engraved notation (Verovio).
  - Page modes, follow mode, transposition.
  - Show or hide parts.
  - Fix misread notes on the page.
- **Practice:**
  - A mixer per voice: My part / Without me / Balance.
  - Tempo, loops, click and pitch pipe.
  - Backing tracks, lock-screen controls.
  - Practice-track export (M4A or WAV).
- **Annotations:** Apple Pencil ink, stamps and text in layers, anchored to
  bars so they survive re-layout. Exports as an annotated PDF.
- **Data:**
  - Sync between devices through any folder (iCloud Drive, OneDrive, …).
  - Backups (`.cantorbackup`) and shareable sets (`.cantorset`).
- **Accessibility:** WCAG 2.2 AA in light, dark and sepia themes. Swedish and
  English.

Status: in active development, tested on an iPad mini. See
`docs/IMPLEMENTATION_PLAN.md` (section 7) for what is done, what was
measured and what is not verified yet.

## Documentation

| Document | For |
|---|---|
| [docs/DEVELOPING.md](docs/DEVELOPING.md) | Build, test, run on an iPad |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | How the code fits together |
| [AGENTS.md](AGENTS.md) | Rules for contributors and AI agents |
| [CONTRIBUTING.md](CONTRIBUTING.md) | How to contribute, and the CLA |
| [docs/README.md](docs/README.md) | Index of all docs: requirements, plan, roadmap, decisions, spikes |

## Quick start

```sh
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"
cargo test --workspace                     # core crates
(cd app/ui && npm ci && npm run build)     # UI
(cd app/src-tauri && cargo tauri dev)      # desktop window
```

For iPad builds, see `docs/DEVELOPING.md`.

## Layout

```
crates/score       MusicXML/MXL parse, model, transpose, edits
crates/timeline    unfold repeats, tempo map, timemap, channel plan
crates/audio       scheduler, SoundFont synth, mixer, backing tracks
crates/library     library store, setlists, bundles, backup, forScore
crates/annotations annotation model, annotated PDF
crates/sync        folder sync between devices
crates/lyrics      lyric phrases for singing
crates/vocal       singing engine adapters (parked)
crates/ocr         on-device text recognition (Apple Vision)
crates/omr         PDF -> MusicXML on a desktop (Audiveris)
app/src-tauri      Tauri 2 backend, iOS integration
app/ui             Svelte 5 UI, pdf.js, Verovio
fixtures/          public test inputs
tools/             CI gates, UI harness, OMR and singing tools
docs/              requirements, plan, roadmap, decisions
```

## Notes from PDFs and forScore files

forScore files and plain PDFs contain no note data, so Cantor reads the notes
with optical music recognition on a desktop:

```sh
# once: Audiveris (https://github.com/Audiveris/audiveris/releases) and the
# scan-cleanup helper (tools/omr/README.md)
cargo run --release -p cantor-omr -- convert scores/*.4sc scores/*.4ss scores/*.pdf --out scores/out --set "My set"
# guide for choir directors: docs/guides/pdf-to-cantor.md
```

## Licence

Cantor is free software under the **GNU Affero General Public License,
version 3** (`LICENSE`). It adds a section 7 permission so it can be
distributed through app stores (`LICENSE-EXCEPTIONS.md`). Third-party
components keep their own licences (`THIRD_PARTY_NOTICES.md`, checked by
`tools/licenses.py` in CI). Contributions need the CLA in `CONTRIBUTING.md`.
Sheet music in `scores/` is private and is not part of the project.
