# Security

## Reporting a vulnerability

Please do not open a public issue for security problems. Use GitHub's
private vulnerability reporting ("Report a vulnerability" under the
repository's Security tab). You should get an answer within 7 days.

## Scope

Cantor works offline: the client makes no network calls. The parts most
worth checking are:

- **Files from outside**, through Files, AirDrop and the share sheet:
  `.mxl`, `.musicxml`, `.pdf`, `.4sc`, `.4ss`, `.cantorset` and
  `.cantorbackup`. These are parsed by the `score`, `library` and
  `annotations` crates and by pdf.js. Look for crashes, path traversal (zip
  entries, the `cantor://` scheme) and very large inputs.
- **The sync folder** (`crates/sync`): logs and blobs written by other
  devices.
- **Restoring a backup**, which replaces library files.

Known safeguards:

- Zip entries and the `cantor://` scheme reject `..` and `/` in names.
- Backups carry a sha256 manifest.
- Saves are atomic (write a temp file, then rename).

## CI

CI runs on the owner's own Mac (a self-hosted runner). It runs only for the
owner's pushes and for pull requests from branches of this repository:

- Pull requests from forks never run on it. The workflow's job conditions
  skip them, and the runner's own job-started hook
  (`tools/ci/runner_guard.sh`, installed outside the checkout) refuses any
  other repository, actor or event before the first step.
- The `GITHUB_TOKEN` is read-only, only GitHub's own actions are allowed,
  and they must be pinned to a commit SHA.
- Outside contributors' workflow runs need the owner's approval.

