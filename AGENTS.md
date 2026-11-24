# Agent and contributor instructions

Read this before changing code. It holds the rules and commands that are easy
to get wrong. Architecture: `docs/ARCHITECTURE.md`. What to build next:
`docs/ROADMAP.md` and section 7 of `docs/IMPLEMENTATION_PLAN.md`.

## Project in one paragraph

ChoirBird (formerly Cantor) is an offline choir practice app and score reader
for choir singers. Bundle id `se.nikielhard.choirbird`. The internal code
name stays `cantor` (crates, repo, Xcode project); every user-visible
string says ChoirBird. BandBird and ConductorBird are later sister apps.

- It shows the original PDF, or a re-engraved MusicXML score.
- It plays every part with its own instrument, with a cursor on the PDF.
- It supports annotations, setlists, folder sync, forScore import and export,
  and practice-track export.

Stack:

- Rust core crates (`crates/*`).
- A Tauri 2 backend (`app/src-tauri`).
- A Svelte 5 UI with pdf.js and Verovio (`app/ui`).

The main target is iPad (iPadOS); Mac also works. The licence is AGPL-3.0
with an app store permission (`LICENSE-EXCEPTIONS.md`).

## Setup

```sh
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"
# Rust stable, Node 20+, npm. iOS builds: Xcode, `cargo install tauri-cli`, xcodegen.
(cd app/ui && npm ci)
```

macOS has no `timeout` command. Use the tool's own timeouts.

## Commands

| Task | Command |
|---|---|
| Format | `cargo fmt --all` and `(cd app/src-tauri && cargo fmt)` |
| Lint (must be clean) | `cargo clippy --workspace --all-targets -- -D warnings` and `(cd app/src-tauri && cargo clippy -- -D warnings)` |
| Core tests | `cargo test --workspace` |
| App backend tests | `(cd app/src-tauri && cargo test --lib)` |
| Requirement traceability | `bash tools/trace.sh` |
| Coverage gate (≥ 85% per core crate) | `bash tools/coverage.sh` |
| Dependency licences | `python3 tools/licenses.py`; after a dependency change also `--report THIRD_PARTY_NOTICES.md` |
| UI build | `(cd app/ui && npm run build)` |
| UI scenarios + accessibility audit | `OUT=$TMPDIR/pw bash tools/ui_harness/run.sh harness`, `a11y` or `all` (each run picks a free port) |
| iPad build | see `docs/DEVELOPING.md` |

**Before every commit:** fmt, clippy with `-D warnings`, and the tests of
what you touched. Run the UI harness if you touched `app/ui` or
`tools/ui_harness`. CI runs all of these (`.github/workflows/core.yml`).

## Rules

### Never commit

- `scores/`: private sheet music. Copyrighted scores must never enter git.
- `app/src-tauri/resources/scores/`: copies of private scores bundled into
  local builds.
- `app/src-tauri/gen/apple/assets/`.
- `design/`: a parallel design workspace.
- Secrets, signing keys, provisioning profiles.

Test inputs that must be public go in `fixtures/` and must be your own work
or public domain. Example: `fixtures/ocr/title_page_sv.png` is a synthetic
page drawn by a script.

### Git

- Add files by path. Never `git add -A` or `git add .`, because other work may
  be in the tree.
- Push with `git pull --rebase --autostash && git push`.
- Commit messages say what changed and how it was checked.

### Tests first, named by requirement

- Every requirement in the PRD has an id (`LIB-03`, `PLAY-02`, `SET-01`,
  `DATA-02`, ...).
- Rust tests are named `<id>_<what>` in lower case, for example
  `lib_03_title_is_the_biggest_text_near_the_top`.
- UI harness steps are named `step('<id> …')`.
- `tools/trace.sh` counts both. With `--strict` it fails if a P0 requirement
  has no test; today 3 parked singing P0s have none.
- Write the failing test, then the code.

### Architecture rules

- **The original PDF is the truth.** Notation is an option beside it.
- **All time comes from `cantor_timeline::Timemap`.** Do not compute note
  times anywhere else.
- **Core crates stay platform-free.** Put platform code in `app/src-tauri`
  behind `#[cfg(target_os = "...")]`. The only exception is the Apple Vision
  module in `crates/ocr`.
- **The client makes no network calls.** The UI harness blocks the network
  and fails on any outside request.
- **No `window.prompt` / `confirm`.** WKWebView does not show them; use
  `ask()` / `confirmBox()` from `lib/prompts.svelte.ts`.
- **iOS shows the keyboard only for a focus inside a user gesture.** For
  dialogs opened after an `await`, call `primeKeyboard()` in the gesture (see
  `prompts.svelte.ts`).
- **Every user-visible string lives in `lib/i18n.svelte.ts`**, in Swedish
  and English.
- **Accessibility:** WCAG 2.2 AA.
  - Every control needs a name.
  - Use the skin tokens (`--accent`, the ChoirBird coral: `#B83E26` light, `#A8381F` sepia, `#FF7A5C` dark), and `--accent-ink` for text on accent.
  - Sizes are in rem.
  - Honour `prefers-reduced-motion`.
  - The `a11y` harness must report 0 findings.
- **Animation:** use transforms only, and do no layout reads during drags
  (see `lib/reorder.ts`). Target 60 fps.

### Licences

- New dependencies must pass `tools/licenses.py`: MIT, Apache-2.0, BSD, ISC,
  Zlib, MPL-2.0, Unicode, CC0 or similar.
- **Do not add GPL, AGPL or LGPL code to the client** without a reviewed
  entry in `REVIEWED` in `tools/licenses.py`.
- **homr** (AGPL-3.0): the product owner accepted AGPL for on-device note
  recognition (2026-11-23). The Rust port lives only in `crates/homr`, which
  is AGPL-3.0-only, keeps homr's copyright notice and says it is a port. Our
  section 7 app store permission does not cover it (it is not ours to give).
  Decision 2026-11-24: App Store builds may contain it once the source of
  that exact build is public (homr's `LICENSE_FAQ.md`). Do not copy homr code
  into other crates. See `crates/homr/README.md`.
- Regenerate `THIRD_PARTY_NOTICES.md` after any dependency change. CI fails
  when it is stale.
- Contributions are accepted under the CLA in `CONTRIBUTING.md`.

### iOS specifics

- **Linking:** the app links the Rust code as a static `libapp.a`. Apple
  frameworks named only by Rust crates (for example Vision, through
  `objc2-vision`) are **not** linked automatically. Add them under
  `dependencies` in `app/src-tauri/gen/apple/project.yml`, then run
  `xcodegen generate` there. Check with `otool -L` on the built app binary.
- **ONNX Runtime (`ort` 2.x)** defaults to a device policy that uses CoreML
  (fp16) on Apple even when you ask for nothing. Pin
  `AutoDevicePolicy::PreferCPU` when exact results matter.
- **Apple Vision** compiles its model on the first call after an install or
  reboot. That call can take a minute; later calls take under a second.

## Where to look

| Need | File |
|---|---|
| Requirements (ids, priorities, acceptance) | `MusicXML Score Reader — PRD & Test-Driven Development Plan.md` |
| Status, measurements, what is not verified | `docs/IMPLEMENTATION_PLAN.md` section 7 |
| Next phases, what waits for the firm | `docs/ROADMAP.md` |
| Business and licence decisions | `docs/monetization-feasibility.md` |
| forScore file formats | `docs/formats/forscore.md` |
| Note-recognition engines | `docs/omr-engine-comparison.md` |
| Singing engines | `docs/spikes/` |

## Reporting

When you finish a task, say what changed, how it was checked (the test names
and the observed result), and **what is not verified**. Examples: not tried on
a real device, not tried with VoiceOver. Keep the plan in
`docs/IMPLEMENTATION_PLAN.md` honest.
