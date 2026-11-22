# forScore extras: what Cantor still needs

Cantor already imports forScore files:

- `.4sc` (one score): the PDF, the title and the handwritten ink.
- `.4ss` (setlist): the order and every score in it.

The PRD asks for more (LIB-07, LIB-08). All of it depends on **sample files**. forScore's format is not published, so each feature has to be read from a real file first. The importer keeps every field it does not understand, so nothing is lost meanwhile. Nothing is shown for those fields yet either.

## What is missing

| # | Feature (PRD) | Why it matters to a singer | What Cantor needs to learn |
| --- | --- | --- | --- |
| 1 | **Text and stamp annotations** (LIB-07) | Breath marks, dynamics and "watch the conductor" notes written in forScore should come along | Whether forScore stores them as separate items (text, stamp type, position) or burns them into the ink picture. If separate, Cantor turns them into its own editable text and stamps |
| 2 | **Bookmarks** (LIB-07) | Jump to "verse 2" or "Coda" by name | Name and page (and the page range, if the bookmark spans one) |
| 3 | **Links** (LIB-07) | Tap the D.S. sign to jump to the segno | Source area, target page and target area |
| 4 | **Audio and MIDI attachments** (LIB-07) | A practice recording or a MIDI file attached to the score | How the file is embedded (bytes in the archive or a reference) and its name. Cantor would play it as an extra mixer channel (PERF-04 backing track) |
| 5 | **Metadata** (LIB-07, LIB-03) | Composer, genre, tags, rating, difficulty, key and tempo from your forScore library show up in Cantor's search | Key names and value types (for example: rating 1-5 or 0-100? key as text or a number?) |
| 6 | **Setlist placeholders** (LIB-08) | An item in the concert order that has no score yet ("Speech", "Encore") | How forScore marks an item with no file |
| 7 | **Per-item start page** (LIB-08) | A setlist item starting on page 3 of a book | The field name in the setlist item. Your `CMB 40.4ss` has none |
| 8 | **`.4sb` full backup** (LIB-08) | Move a whole forScore library (all scores, setlists, metadata) into Cantor in one go | The whole file layout. No sample yet |

## Sample files to send

Small files are fine. One page is enough. What matters is that the feature is in the file.

1. **Annotations:** a `.4sc` where you have added:
   - a **text box** (e.g. "andas")
   - two or three **stamps** (a breath mark, a dynamic, a fingering number)
   - one ordinary pen stroke, so we can see whether stamps end up inside the ink picture or apart from it
2. **Bookmarks and links:** a `.4sc` with **two bookmarks** (one single page, one page range) and **one link** (a tap area that jumps to another page).
3. **Attachments:** a `.4sc` with an **audio track** attached (any short recording) and, if forScore allows it, a **MIDI file**.
4. **Metadata:** a `.4sc` with **composer, genre, tags, rating, difficulty, key and tempo** all filled in. Use values you can recognise, e.g. composer "Testkompositör", rating 4, difficulty 2.
5. **Setlist:** a `.4ss` that contains:
   - a **placeholder** item
   - a score that starts on a **later page** (if forScore lets you set one)
   - two scores
6. **Backup:** a **`.4sb`** backup of a small test library (a few scores and one setlist) rather than your full library. If you can only send a full one, that works too. It stays on this machine in `scores/` and is never committed.

### How to export from forScore

- **One score (`.4sc`):** open the score, tap the **share** icon, choose **"forScore file"** (or "Share as forScore"), then AirDrop it or save it to Files.
- **A setlist (`.4ss`):** in the setlist menu, swipe or long-press the setlist, choose **Share**, then the forScore format.
- **A backup (`.4sb`):** forScore's **main menu, then Tools, then Backup**. Create a backup, then share or save the `.4sb` file.

The menu names can differ a little between forScore versions. Look for **Share** and **Backup**.

Put the files in `scores/` on the Mac, or AirDrop them to the Mac. Tell me which file holds which feature.

## What happens when they arrive

For each sample, Cantor's importer gets:

1. The format notes in `docs/formats/forscore.md`.
2. A test named after the requirement (`lib_07_…`, `lib_08_…`) that reads the sample and checks the values you entered.
3. The import itself.

The import does this for each kind of data:

- **Annotations:** become Cantor annotations, editable and on their own layer named "forScore".
- **Bookmarks:** appear in a page menu.
- **Links:** become tap areas.
- **Attachments:** become a mixer channel.
- **Metadata:** goes into the library and search.
- **Setlist details:** placeholders and start pages are kept.
- **A backup:** imports everything into the library in one step.

Until then Cantor keeps every imported forScore file unchanged in a hidden `.forscore` folder in its library (`.4sc` from this version on; `.4ss` when it arrives through the share sheet, Files or Importera). A later Cantor version can then import these fields without asking you to export again. Files imported before this change were not kept and would need exporting again.
