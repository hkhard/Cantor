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

CI (`.github/workflows/core.yml`) runs on a self-hosted macOS arm64 runner with label `cantor`,
installed at `~/dev/cantor-runner` as a launchd service (`./svc.sh status|stop|start`).

Tests are named after PRD requirement IDs (`lib_01_...`, `play_02_...`). Write the failing test first.
