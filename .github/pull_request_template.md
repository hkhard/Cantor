## What changed

## Requirement(s)

PRD ids, for example LIB-03, SET-01:

## How it was checked

- [ ] Tests named after the requirement (`<id>_…`) fail before and pass after
- [ ] `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` (workspace and `app/src-tauri`)
- [ ] UI harness / a11y audit if `app/ui` changed (0 findings)
- [ ] `tools/licenses.py` passes; `THIRD_PARTY_NOTICES.md` regenerated if dependencies changed

## Not verified

(Real device, VoiceOver, other platforms, …)

## Licence

- [ ] I agree to the Cantor CLA, version 1 (`CONTRIBUTING.md`). Needed once per contributor.
- [ ] No private or copyrighted scores added (`scores/` stays out of git).
