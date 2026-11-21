# forScore file formats (spike S5)

Reverse-engineered on 2026-11-21 from three files supplied by the product owner
(kept locally in `scores/`, not committed, see `.gitignore`):

| File | Size | Content |
| --- | --- | --- |
| `Christmas Day - F dur.4sc` | 311 KB | one score, 6-page PDF, no annotations |
| `CMB 40.4ss` | 10.9 MB | setlist of 10 scores with embedded PDFs |
| `En glad calypso om våren TTBB.pdf` | 225 KB | plain PDF, 2 pages, A4, scanned (Photoshop) |

## Container

Both `.4sc` and `.4ss` are **gzip** (magic `1f 8b`, deflate, no filename) wrapping an
**Apple binary property list** (`bplist00`). No encryption, no zip.

## `.4sc` (single score)

The plist root is a flat dictionary. One key is the embedded PDF; the rest are
per-document and per-page entries keyed by a pipe-separated string:

```
"<pdf file name>|<field>"            document-level field
"<pdf file name>|<page>|<field>"     page-level field, page is 1-based
"pdfData"                            raw PDF bytes (starts with %PDF-)
```

Fields seen:

| Key pattern | Type | Example | Meaning (inferred) |
| --- | --- | --- | --- |
| `X.pdf\|title` | string | `Christmas Day - F dur` | score title (metadata) |
| `X.pdf\|N\|rect` | string, NSRect | `{{19, 34}, {743, 971.3}}` | crop / visible rect of page N in points, forScore's per-page crop |
| `X.pdf\|N\|zoom` | real | `1.0336` | zoom factor for page N |
| `X.pdf\|N\|offset` | string, NSPoint | `{6.5, 17.66}` | scroll offset of page N |
| `X.pdf\|N\|trOffset` | string, NSPoint | `{19.64, 35.14}` | offset, probably used in a "transformed" (half-page or two-up) view |

Geometry strings use Cocoa's `NSStringFromRect` / `NSStringFromPoint` syntax:
`{{x, y}, {w, h}}` and `{x, y}`.

Pages 7 and 8 have entries while the PDF has only 6 pages. They contain defaults
(`{{0, 0}, {768, 1004}}`, i.e. an iPad portrait viewport). So the per-page keys can
outnumber the pages, and the importer must keep them without complaint.

Not present in this sample (expected in others, to be added when samples arrive):
annotations, bookmarks, links, audio/MIDI attachments, composer, genre, tags, rating,
difficulty, key, tempo. The importer keeps **every unknown key** verbatim (LIB-07).

## `.4ss` (setlist)

The plist is an **NSKeyedArchiver** archive (`$archiver = NSKeyedArchiver`,
`$version = 100000`). `$top` has two roots:

- `setlist`: `NSMutableArray` of `NSMutableDictionary`, one per item, in setlist order:
  - `Title` (string)
  - `FilePath` (string, the PDF file name, joins to the file entry)
  - `Identifier` (UUID string)
- `files`: `NSArray` of `NSDictionary`. Each is the same flat dictionary as a `.4sc` root
  (`pdfData` plus `X.pdf|...` keys).

Items join to files through `FilePath` = the `X.pdf` prefix of the file's keys.
Class names seen: `NSMutableDictionary`, `NSDictionary`, `NSMutableArray`, `NSArray`.
Placeholders (setlist items with no file) are not in this sample. The importer models
an item with no matching file as a placeholder.

## `.4sb` (backup)

No sample yet. LIB-08 backup import stays open until one is provided.

## Open points for more samples

1. A `.4sc` with ink, text and stamp annotations, plus bookmarks and links.
2. A `.4sc` with an audio track or MIDI attachment.
3. A `.4ss` with a placeholder and a per-item start page.
4. A `.4sb` backup.
5. A score with composer, genre, tags, rating and difficulty filled in.
