# Cantor

MusicXML-aware score reader (forScore-class) with per-part playback, mixer and sung lyrics.

- PRD: `MusicXML Score Reader — PRD & Test-Driven Development Plan.md`
- Implementation plan: [docs/IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md)

## Layout

```
crates/score      MusicXML/MXL parse, model, transpose
crates/timeline   unfold repeats, tempo map, timemap
crates/audio      scheduler, synth, mixer, offline render
crates/lyrics     syllables, melismas, phrases
crates/vocal      VocalProvider trait + adapters, cache
crates/library    SQLite, setlists, sync, backup, forScore import
crates/omr        PDF/.4sc/.4ss -> MusicXML (Audiveris OMR) -> MIDI
app/              Tauri shell + Svelte UI (not started)
fixtures/         corpus + expected outputs
tools/            oracle, traceability
```

## Develop

```
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
tools/trace.sh          # requirement -> test coverage
```

## Notes from PDFs and forScore files

forScore files contain only PDFs, so Cantor reads the notes with OMR:

```
# once: Audiveris (https://github.com/Audiveris/audiveris/releases) in /Applications
# or ~/.local/opt, and the scan-cleanup helper (see tools/omr/README.md)
cargo run --release -p cantor-omr -- scores/out scores/*.4sc scores/*.4ss scores/*.pdf
```

Each score gives `<title>.mxl` (MusicXML) and `<title>.mid` (one track per part).

CI (`.github/workflows/core.yml`) runs on a self-hosted macOS arm64 runner with label `cantor`,
installed at `~/dev/cantor-runner` as a launchd service (`./svc.sh status|stop|start`).

Tests are named after PRD requirement IDs (`lib_01_...`, `play_02_...`). Write the failing test first.
