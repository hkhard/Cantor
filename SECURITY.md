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
