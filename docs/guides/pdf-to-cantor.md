# From a PDF to Cantor (for choir directors)

You have a score as a PDF (or a forScore `.4sc` / `.4ss`). This guide turns it
into a Cantor score that plays every voice, with a cursor that follows the
original PDF, and packs it into one file you can AirDrop to the choir.

You need a Mac. It takes about a minute per page.

## 1. Install once

1. **Audiveris** reads the notes. Download the macOS `.dmg` from
   <https://github.com/Audiveris/audiveris/releases> and drag Audiveris to
   Applications.
2. **Cantor's converter.** Ask whoever builds Cantor for you, or, with the
   Cantor source folder open in Terminal:

   ```sh
   cargo install --path crates/omr
   ```

3. Check that everything is there:

   ```sh
   cantor-omr doctor
   ```

   Every line with ✓ is fine. A line with ✗ must be fixed; `doctor` says how.
   A line with ! is optional: for example missing Swedish text files mean
   lyrics are not read (copy the `curl` line it prints into Terminal).

## 2. Convert, one command

```sh
cantor-omr convert ~/Desktop/Ave-verum.pdf ~/Desktop/Lacrimosa.pdf --set "Spring concert"
```

- You can list several PDFs, `.4sc` files or a forScore setlist `.4ss`.
- Files go into a folder called `Cantor` next to the first PDF. Choose another
  with `--out FOLDER`.
- Each piece gets its name from the title printed on the score.
- Per piece you get `<title>.mxl` (the notes), `<title>.pdf` (your original),
  `<title>.measures.json` (where each measure is on the PDF) and `<title>.mid`.
- `--set "Spring concert"` also makes `Spring concert.cantorset`: all the
  pieces in one file, as a setlist.

The command tells you how each piece went. "cursor follows the PDF" means the
measures on the PDF match the notes. Messages are in Swedish when your Mac is
set to Swedish.

### When a piece fails

- **"could not read the notes"**: the scan is too blurry or too small. Scan
  again at 300 DPI in black and white, or split a book into one PDF per piece.
- **"pages skipped"**: those pages had no readable music (often a title page
  or text). The rest of the piece is fine.
- **"the PDF cursor may drift"**: the notes play, but the cursor on the PDF
  may be off. Use the notation view for that piece.

Note recognition is never perfect. Listen through each voice before sending.

## 3. Send to the choir

AirDrop `Spring concert.cantorset` to each iPad (or share it through Files).
On the iPad, open it with Cantor: the setlist and its pieces appear in the
library.
