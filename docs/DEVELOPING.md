# Developing Cantor

How to build, test and run Cantor. The rules (what never to commit, naming,
licences) are in `AGENTS.md`. How the code fits together is in
`docs/ARCHITECTURE.md`.

## Requirements

| Tool | Version used | For |
|---|---|---|
| macOS | 26+ (27 used) | iOS builds, Apple Vision tests |
| Rust | stable (rustup) | everything |
| Node + npm | 20+ | UI |
| Xcode | 26+ (iOS SDK 27.2 used) | iOS builds |
| tauri-cli | 2.x (`cargo install tauri-cli`) | `cargo tauri ios …` |
| xcodegen | `brew install xcodegen` | regenerating the Xcode project |
| Python 3 | 3.9+ | `tools/licenses.py`, OMR tools |
| cargo-llvm-cov | installed by `tools/coverage.sh` | coverage gate |

```sh
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"
git clone https://github.com/hkhard/Cantor && cd Cantor
(cd app/ui && npm ci)
cargo test --workspace
```

Linux works for the core crates (`cargo test --workspace`). The Apple Vision
test only runs on macOS; elsewhere `recognize` returns `Unsupported`.

## Core crates

```sh
cargo test --workspace                       # all requirement tests
cargo test -p cantor-library lib_03          # one crate, filtered by requirement id
cargo clippy --workspace --all-targets -- -D warnings
bash tools/trace.sh                          # requirement -> test map (--strict fails on P0 gaps)
bash tools/coverage.sh                       # fails if a core crate is under 85%
```

## App backend and UI

```sh
(cd app/src-tauri && cargo test --lib && cargo clippy -- -D warnings)
(cd app/ui && npm run dev)                   # UI in a browser, backend calls fail (no Tauri)
(cd app/ui && npm run build)                 # production bundle for Tauri
(cd app/src-tauri && cargo tauri dev)       # desktop window with the real backend (starts vite itself)
```

### Swedish and English (NFR-10)

`node tools/i18n_check.mjs` is the test `nfr_10_sv_en_localisation_complete`.
It fails if a key in `app/ui/src/lib/i18n.svelte.ts` lacks Swedish or English,
if `t()` names a missing key, or if a Svelte file has literal user-visible text
(text between tags, `aria-label`, `title`, `placeholder`, `alt`, `label`, or a
literal string passed to `ask`/`confirmBox`/`alert`). Names such as Cantor and
PDF are allowed (see `ALLOW` in the script). CI runs it in the UI job.

### UI harness (scenarios and accessibility)

`tools/ui_harness` runs the real UI in Playwright WebKit at iPad mini size.
The Tauri backend is replaced by an in-memory mock (`mock.js`), and all
network traffic is blocked.

```sh
OUT=$TMPDIR/pw bash tools/ui_harness/run.sh harness   # feature scenarios
OUT=$TMPDIR/pw bash tools/ui_harness/run.sh a11y      # axe WCAG 2.2 AA, keyboard, 200% text
OUT=$TMPDIR/pw bash tools/ui_harness/run.sh all
```

Screenshots are written to `$OUT/shots`, and a failing step saves
`fail-<step>.png`.

**Scores:** the harness needs no private scores. It uses the public fixture
`fixtures/scores/mozart-lacrymosa/` (MusicXML unzipped from the `.mxl`, the
PDF, and `measures.json`). Regenerate the measure map with
`cargo run -q -p cantor-score --example layout_boxes -- <mxl> > measures.json`.
Forks and outside contributors can run it as is (done 2026-11-23).

## iPad

The app is iPad-only today (`tauri.conf.json`: `bundle.iOS`).

- **Signing:** builds are signed with the development team in
  `app/src-tauri/tauri.conf.json` (`bundle.iOS.developmentTeam`) and
  `gen/apple/project.yml` (`DEVELOPMENT_TEAM`).
- **Your own builds:** set your own team id and bundle identifier
  (`identifier`), then regenerate the project:

```sh
cd app/src-tauri/gen/apple && xcodegen generate
```

Build and install on a connected device:

```sh
(cd app/ui && npm run build)
cd app/src-tauri
cargo tauri ios build --target aarch64 --export-method debugging \
  --config '{"build":{"beforeBuildCommand":""}}'
xcrun devicectl list devices
xcrun devicectl device install app --device <UDID> gen/apple/build/arm64/Cantor.ipa
xcrun devicectl device process launch --device <UDID> <bundle id>
```

- The first launch after install can fail with a signature error. Retry
  after a few seconds.
- Launch fails while the device is locked.

### Notes on the iPad build

- **Demo scores:** `tools/bundle_demo_scores.sh` (run by `tools/ipad.sh`)
  fills `resources/scores` with exactly what `tools/demo-scores.txt` lists.
  Today that is Lacrymosa only. Run it before a manual `cargo tauri ios build`.
  Everything else is imported through Files or AirDrop.
- **Linking frameworks:** Apple frameworks used from Rust must be listed in
  `gen/apple/project.yml`, because the Rust code is a static library. Check
  the result with:
  `unzip -o Cantor.ipa -d x && otool -L x/Payload/Cantor.app/Cantor`.
- **Reading data off the device:**
  `xcrun devicectl device copy from --device <UDID> --domain-type appDataContainer --domain-identifier <bundle id> --source Documents/.perf.log --destination perf.log`.
  The log records timings and memory (`perf_mark`) and text recognition runs.

## Note recognition for PDFs (desktop)

PDFs and forScore files have no note data. `cantor-omr` reads them with
Audiveris (installed separately) and writes, per piece, MusicXML, the
original PDF, a measure map and MIDI, named after the printed title:

```sh
cargo run --release -p cantor-omr -- doctor     # what is installed, what is missing
cargo run --release -p cantor-omr -- convert scores/*.4sc scores/*.pdf --out scores/out [--set "Name"]
```

`--set` also writes a `.cantorset` to AirDrop. The old form
`cantor-omr <out-dir> <files>...` still works. The guide for directors is
[guides/pdf-to-cantor.md](guides/pdf-to-cantor.md).

Scan cleanup, Tesseract languages and two-page spreads are covered in
`tools/omr/README.md`. The on-device engine study is in
`docs/omr-engine-comparison.md`; its spike code is in `tools/omr/spike/`.

## Singing (parked)

`tools/soulx/` renders a part with SoulX-Singer on a Mac GPU (MPS) and
checks the pitch. See `docs/spikes/singing-engines.md`. Not part of the app
today.

## CI

`.github/workflows/core.yml` runs on a self-hosted macOS runner:

- **Core job:** fmt, clippy, tests, app backend tests, the licence check,
  traceability and the coverage gate.
- **UI job:** the UI harness and the accessibility audit.

The runner has the label `cantor` and is installed at `~/dev/cantor-runner`
as a launchd service (`./svc.sh status|stop|start`). The core job reads
private forScore samples from the maintainer's machine (`CANTOR_SAMPLES`);
the tests that need them skip when they are missing. The UI job uses only
public fixtures.

## Releasing a build to anyone else

AGPL section 6 applies as soon as anyone else gets a build: they must be able
to get the exact source. Before handing out a build:

1. Make sure the source for that commit is public (see `docs/ROADMAP.md`,
   phase 0).
2. Tag the commit, and check that About → Licence points at it.
3. Run `python3 tools/licenses.py` and confirm `THIRD_PARTY_NOTICES.md` is
   current.

### TestFlight

1. Tag the commit (`git tag -a v0.2.0 -m …`, push the tag).
2. App Store build of the tag in a clean worktree: `tools/ipad.sh release
   app-store-connect` (version = tag, build number = commit count; the
   bundled demo is Lacrymosa only).
3. Upload the archive with Xcode's signed-in account:
   `xcodebuild -exportArchive -archivePath gen/apple/build/cantor-app_iOS.xcarchive
   -exportOptionsPlist upload-options.plist -exportPath out -allowProvisioningUpdates`,
   where the options plist has `method = app-store-connect`,
   `destination = upload`, `teamID = 33ZQ6Q226L`.
4. **Internal testers only** until the repository is public (AGPL 6:
   external testers must be able to get the exact source of their build).

First upload: ChoirBird 0.2.0 (build 0.2.0.110, tag `v0.2.0`), 2026-11-23,
"Upload succeeded". Processing and TestFlight availability are checked in
App Store Connect.
