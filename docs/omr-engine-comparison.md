# OMR engine comparison: homr and oemer vs Audiveris

Date: 2026-11-23. Question: could an on-device engine replace Audiveris for note
recognition, so that a choir member can import a PDF on the iPad without a
desktop or server?

**Short answer:** homr is the only candidate. On clean, typeset scores it reads
notes about as well as Audiveris (85-98% agreement), at about 15 s per page on
a Mac M2 CPU. Peak memory is close to the iPad mini's ~3 GB app limit, and
parts of its output are weak for choir use (it reads every system as "piano",
ignores the tenor 8vb clef, and crashed on 8 of 60 pages). oemer is ruled out:
5-7 GB of memory and about 5 minutes per page, and it agreed poorly even on the
easy scores. Recommendation: keep Audiveris on the desktop for now, and start a
time-boxed homr-on-iPad spike (ONNX Runtime with the CoreML provider) to measure
real memory and speed before building on it. See the end of this document.

## What was measured

- 60 pages from 13 scores (`scores/out/pdf/*.pdf`, rendered at 300 DPI to PNG).
  These are the product owner's real choir sheets: clean engravings, scanned
  book pages, two-page spreads, and one low-resolution phone photo of a
  "PREVIEW" sheet.
- homr 0.7.1 ran on all 60 pages. oemer 0.1.5 ran on page 1 of each score
  (13 pages, plus Christmas Day page 2). It was stopped there because its memory
  alone rules it out (see below).
- Machine: Mac M2, 8 GB RAM. One engine process at a time, `nice -n 15`,
  BLAS/OpenMP threads capped at 4. Times are wall clock from
  `/usr/bin/time -l`. The machine was also running other work, so treat times
  as about ±30%.
- homr ran with its defaults on macOS: segmentation net on the CoreML provider
  (fp16), transformer encoder and decoder on the CPU. oemer was forced onto the
  CPU provider, because its CoreML path splits the model into about 90 pieces
  and was much slower (one trial page took over 7 min).
- Reference: Audiveris output in `scores/out/*.mxl` / `*.musicxml`.
  **Audiveris is not ground truth.** "Agreement" means "agrees with what Cantor
  gets today", not "correct". The spot checks below show cases where homr is
  right and Audiveris is wrong.
- Script: `tools/omr/compare_engines.py` (method in its docstring). In short:
  - Each staff becomes a sequence of MIDI pitches in reading order. Engine
    staves are matched to Audiveris staves by best fit.
  - Agreement = 1 - edit distance / length, weighted by note count.
  - Also reported: agreement allowing a whole-staff octave shift (so a missed
    tenor 8vb clef does not count), pitch-class agreement, and pitch-histogram
    overlap.
  - Only pages that Audiveris recognised (per `<stem>.measures.json`) are
    compared.

## Resources: what decides on-iPad feasibility

| | homr | oemer | Audiveris (today) |
|---|---|---|---|
| **Seconds per page** (M2 CPU) | **median 15 s** (4-31 s) | median 310 s (270-430 s) | runs on desktop |
| **Peak RSS** | **median 1.4 GB, max 2.0 GB** | median 3.8 GB, max 4.1 GB | several GB (Java) |
| **Peak memory footprint** (macOS, incl. compressed/CoreML) | **median 2.7 GB, max 3.1 GB** | 5.7-7.3 GB | n/a |
| Pages finished without crashing | 51/60 | 9/14 | n/a |
| Model files | 129 MB (segnet fp16 29 MB, encoder 53 MB, decoder 47 MB, ONNX) | 144 MB (2 U-Nets 109 MB ONNX + 4 sklearn models 35 MB) | none |
| Licence | AGPL-3.0 | MIT | AGPL-3.0 |
| Runtime | Python, ONNX Runtime, OpenCV | Python, ONNX Runtime, OpenCV, scikit-learn | Java, Tesseract |

Notes:

- homr's numbers include the Python interpreter, OpenCV, and an OCR model it
  loads for titles. A native port would probably use less, but that is **not
  verified**. On the Mac, homr's peak footprint (up to 3.1 GB) is at the iPad
  mini's roughly 3 GB app limit, so as measured it **does not fit with any
  margin**.
- The 15 s/page is on an M2 CPU. On the iPad, CoreML/Neural Engine for the
  transformer could be faster or slower. **Not measured.** homr's own
  `--coreml-encoder` option says compiling the model takes 26-60 s at
  startup.
- oemer needed two fixes before it ran at all: numpy < 2 and OpenCV < 5
  (`np.int` was removed, and `HoughLinesP` changed shape). The project looks
  unmaintained.

## Note agreement with Audiveris

### Page 1 of each score, both engines (same pages)

| Score | Audiveris notes (meas.) | homr notes (meas.) | homr agr. | homr agr. (octave ok) | oemer notes (meas.) | oemer agr. |
|---|---|---|---|---|---|---|
| Bacchi Vaggvisa | 186 (13) | 192 (13) | 85% | 85% | 128 (17) | 14% |
| Christmas Day | 170 (10) | 172 (10) | 98% | 98% | 179 (12) | 42% |
| Do you hear (phone photo) | 165 (12) | crash | - | - | crash | - |
| En glad calypso | 135 (9) | 142 (9) | 55% | 68% | 128 (20) | 19% |
| Glimmande Nymf | 175 (18) | 181 (18) | 91% | 91% | 115 (14) | 14% |
| I bröllopsgården (p2, see below) | 31 (4) | 300 (40) | 10% | 10% | crash | - |
| Island | 275 (16) | 281 (16) | 52% | 95% | 255 (26) | 26% |
| Jag och min far | 60 (24) | 135 (17) | 11% | 28% | 131 (17) | 8% |
| Olaf Trygvason | 224 (17) | 225 (17) | 97% | 97% | 151 (30) | 17% |
| Stad i ljus | 122 (8) | 126 (8) | 32% | 32% | 126 (10) | 12% |
| Supa klockan | no reference | 211 (18) | - | - | crash | - |
| Kung Liljekonvalje | 174 (11) | 163 (11) | 32% | 58% | crash | - |
| Är jag född | 116 (4) | 116 (4) | 51% | 100% | crash | - |
| **Mean over scores with a reference** | | | **51%** | **63%** | | **13%** |

oemer gets the right rough pitch range (histogram overlap 39-93%), but its
note order and staff assignment are mostly wrong. It also often outputs a
single staff for a two-staff system. It is not close.

### homr on every page Audiveris recognised

| Score | Pages | Audiveris notes / meas. | homr notes / meas. | Agreement | Octave ok | Pitch class | s/page | Peak RSS MB |
|---|---|---|---|---|---|---|---|---|
| Bacchi Vaggvisa | 1 | 186 / 13 | 192 / 13 | 85% | 85% | 85% | 14.5 | 1165 |
| Christmas Day | 6 | 1112 / 53 | 1348 / 53 | 81% | 81% | 81% | 11.6 | 1896 |
| Do you hear (phone photo) | 9 | 2798 / 141 | 1283 / 287 (5/9 pages ok) | 7% | 7% | 13% | 21.0 | 1971 |
| En glad calypso | 2 | 387 / 25 | 398 / 25 | 52% | 65% | 77% | 13.1 | 1909 |
| Glimmande Nymf | 2 | 350 / 40 | 370 / 40 | 88% | 88% | 88% | 10.9 | 1565 |
| I bröllopsgården | 5 | 938 / 80 | 1046 / 177 | 20% | 27% | 31% | 13.2 | 1725 |
| Island | 1 | 275 / 16 | 281 / 16 | 52% | 95% | 95% | 12.4 | 1552 |
| Jag och min far | 4 | 366 / 58 | 842 / 63 | 11% | 17% | 18% | 14.5 | 1552 |
| Olaf Trygvason | 4 | 862 / 62 | 871 / 62 | 96% | 96% | 96% | 18.1 | 1579 |
| Stad i ljus | 2 | 137 / 11 | 271 / 26 | 21% | 21% | 21% | 14.6 | 1311 |
| Kung Liljekonvalje | 4 | 665 / 38 | 765 / 38 | 26% | 48% | 49% | 15.4 | 1455 |
| Är jag född | 3 | 382 / 12 | 382 / 12 | 47% | 100% | 100% | 14.3 | 1409 |

On 6 of 12 scores, homr matches Audiveris at 85-100% once the octave is
allowed for, with the same measure count. The low scores fall into three
groups:

1. **Tenor clef octave** (Island, Är jag född, part of Calypso and Kung
   Liljekonvalje). homr reads the treble clef with the small 8 below as a plain
   treble clef, so tenor lines come out one octave too high. With that allowed
   for, Är jag född is 100% and Island 95%. This is easy to fix afterwards
   (shift the staff when the clef has an 8 under it, or ask the user), but homr
   does not do it.
2. **Pages where Audiveris is the one that's wrong** (Jag och min far,
   I bröllopsgården, Stad i ljus). See the spot checks.
3. **Bad input** (Do you hear: a low-resolution phone photo of a two-page
   preview spread with a watermark). homr crashed on 8 of 18 pages
   (`cv2.resize` assertion on a zero-width staff crop). On the pages that did
   work it made one long staff per system, so the four voices are lost. Neither
   engine is usable on this input.

## Spot checks (Verovio rendering next to the PDF page)

The renders were made with `verovio` from each engine's MusicXML and
compared by eye with the page image. They were not committed (private scores).

- **Island p1 (clean engraving, TB with tenor 8vb clef).** Audiveris and homr
  have the same notes and rhythms as the print in the measures checked. homr
  drops the 8vb (so tenor is an octave high), loses the lyrics, and labels the
  system "Piano". Audiveris keeps the 8vb and lyrics. Audiveris wins, but only
  on clef and lyrics.
- **Bacchi Vaggvisa p1 (old scanned engraving).** Audiveris and homr both
  match the print closely (85% agreement, differences mostly in chords).
  oemer's output is mostly wrong: wrong clefs and key changes mid-line, notes
  far off the staff.
- **Jag och min far p1 (tenor melody line + bass, chord symbols).** homr's
  tenor line (12/8, F. F G E. E...) matches the print. Audiveris turned page 1
  into four parts of mostly rests and lost the first measures, which is why the
  agreement is 11%. **homr is right here and Audiveris is wrong.** homr still
  ignores the 8vb clef.
- **I bröllopsgården p2 (two-page book spread).** Audiveris placed only 4
  measures on this page and produced a garbled first part. homr read 40
  measures with plausible pitches for the left page. It did not split the voices
  correctly (it puts both choir staves into one grand staff and mixes up a
  clef). **Neither is good. homr is closer to the print.**
- **Kung Liljekonvalje p1 (scanned book page with dark gutter).** Audiveris
  made four parts (two empty) for a two-staff system. homr made one grand
  staff, with the right notes in the measures checked, but the tenor octave is
  wrong.

Not verified: no score was checked note by note against the print. Spot checks
cover the first system or two of each page shown.

## Failure modes

**homr**

- Every system becomes a two-staff "Piano" part. Choir part names and the
  number of voices are not detected. Cantor would need its own step to map
  staves to voices (the measure map from `measures.json` currently comes from
  Audiveris).
- Ignores octave clefs (treble 8vb), so TTBB tenor lines are an octave high.
- No lyrics.
- Crashes (8 pages): `cv2.resize` assertion on tiny or zero-width staff crops
  in low-resolution photos, plus "No staffs found" on a lyrics-only page. The
  second one is correct behaviour, but it exits with an error.
- Two-page spreads: reads them, but can merge staves from both pages into one
  system.

**oemer**

- Crashed on 5 of 14 pages: staffline unit-size check, index errors, an
  assertion.
- Wrong clefs and key signatures in the middle of a line, notes far off the
  staff, single-staff output for grand staves.
- 4-7 GB of memory and about 5 minutes per page.
- Needed pinned old versions of numpy/OpenCV just to run.

## Licences

- **homr: AGPL-3.0.** Shipping it inside the iPad app would put the app under
  AGPL (source offer to every user), and the App Store terms have a known
  conflict with GPL-family licences. Today Audiveris (also AGPL) is kept as a
  separate process for exactly this reason (see `IMPLEMENTATION_PLAN.md`).
  Possible ways out: ask the author for a separate licence, use only the
  model weights (check their licence separately, **not verified**), or retrain
  a compatible model. This needs legal review before any on-iPad homr ships.
- **oemer: MIT.** No licence problem, but the engine is not good enough.

## Recommendation

1. **Do not use oemer.** It is too slow, uses too much memory, and its
   accuracy was poor on every page we tested.
2. **Keep Audiveris on the desktop (and the parked server plan) as the
   production path for now.** It is still better on choir-specific structure:
   part names, octave clefs, lyrics.
3. **homr is promising enough for a time-boxed spike on the iPad**, through ONNX
   Runtime with the CoreML provider. Its three ONNX models are 129 MB. On clean
   pages its notes agree with Audiveris (85-100% with the octave allowed for),
   and on some bad scans it is better than Audiveris. The spike should answer:
   - Peak memory on a real iPad mini with native preprocessing (no Python,
     OpenCV only where needed). It must stay well under 3 GB. The Mac
     measurement (1.4-2.0 GB RSS, up to 3.1 GB footprint) is too close.
   - Seconds per page on the iPad, and the first-run CoreML compile delay.
   - Porting cost of homr's non-ML parts (staff detection, dewarping, MusicXML
     building), which are Python/OpenCV.
   - The licence question above. **This blocks shipping**, but not the spike.
4. If homr goes on the iPad, Cantor needs its own post-processing: map staves
   to voices, handle octave clefs (8vb), and handle lyrics if they matter.

## Update: homr's models on the iPad mini (spike, 2026-11-23)

The question was whether homr's memory and speed fit on the iPad. To answer
it, homr's three ONNX models ran natively on the iPad mini (A17 Pro, 8 GB),
inside Cantor, with ONNX Runtime 1.31.1 from Microsoft's NuGet package
(SHA-512 checked against nuget.org). There was no Python. The inputs were
captured from homr on a real page (Olaf Trygvason p1, 300 DPI, 4 staves),
and the outputs were compared with Python's. Code: `tools/omr/spike/`.

| | iPad mini, CPU | iPad mini, CoreML first run | iPad mini, CoreML later runs | Mac M2, CPU |
|---|---|---|---|---|
| Segmentation, one page (9 batches of 8 patches) | 8.3 s | 1.6 s | 0.9 s | 4.8 s |
| Encoder, 4 staves | 1.5 s | 2.6 s | 0.4 s | 0.9 s |
| Decoder, 4 staves (464 tokens, CPU in every run) | 5.9 s (12.6 ms/token) | 6.0 s | 6.0 s | 3.7 s |
| Model loading | 0.2 s | 20 s (CoreML compiles the models) | 4.8 s | 1.1 s |
| **Total for the page** | **16 s** | **30 s** | **12 s** | 10.6 s |
| **Peak memory footprint** | **609 MB** | **1.17 GB** | **1.18 GB** | 0.92 GB |
| Same result as Python | segmentation 100% of pixels, decoder 117/117 tokens | same | same | same |

A second, denser page was also run on the device: Christmas Day p3, 8
staves, typeset with chord symbols, CoreML, first run after install. The
segmentation was identical to Python on every pixel, and the decoder matched
33 of 33 tokens on staff 0. Peak memory was 1.16 GB, the same as the 4-staff
page: segmentation sets the peak, not the number of staves. The whole run
took 26.7 s, of which 18 s was CoreML compiling the models; installing a new
app version clears that cache. Encoder and decoder took 5.6 s for 8 staves.

What this shows:

- **Memory fits.** The models peak at 0.6 GB on the CPU and 1.2 GB with
  CoreML. That is well under the iPad mini's limit. The 2.7–3.1 GB measured
  before was Python, OpenCV and homr's title-OCR model, not the networks.
  Not measured yet: the non-ML steps (staff detection, dewarping) in native
  code. They work on one page image at a time and should add tens of MB, not
  GB.
- **Speed is fine for importing.** A page takes 12–16 s, a 4-page score about
  1 minute. It runs in the background while the PDF is already readable.
  CoreML makes segmentation about 10× faster. The decoder (one token at a
  time) is the slow part and stays on the CPU because its shapes change every
  step.
- **Results are identical** to homr in Python on the Mac, with the fp32 CPU
  models (decoder) and with CoreML (segmentation, encoder). The pitfall:
  `ort` silently uses CoreML in fp16 by default on Apple. That changed 55 of
  117 decoder tokens until it was pinned to the CPU.
- **App size:** the app grew by 9 MB (ONNX Runtime, static). The models add
  129 MB, either bundled or downloaded on first use.

Not covered by the spike:

- The non-ML half of homr is not ported: about 6,000 lines of Python with
  OpenCV and numpy.
  - The biggest parts are staff detection (1,000 lines), the MusicXML writer
    (1,300), bounding boxes (570) and dewarping (500).
  - They use about 25 OpenCV functions: morphology, contours, `minAreaRect`,
    `remap`, `resize`, thresholds.
  - Rough estimate, not measured: 2–4 weeks to port to Rust (`imageproc`, or
    OpenCV's iOS framework), plus tests against homr's output page by page.
- Heat and battery over a long score.
- Only 2 pages were run on the device (4 and 8 staves, both clean typeset
  scores). Scans and phone photos go through the same models, so memory and
  time should be similar, but that was not run.
- Older or smaller iPads.

### Licence, read from homr's `LICENSE_FAQ.md` (checked 2026-11-23)

- **Code: AGPL-3.0.** No commercial licence is offered. The FAQ says:
  "nobody can relicense the code on their own". It asks people not to email
  about licensing. A port of homr's Python code to Rust would be a derivative
  work, so it is also AGPL.
- **Model weights: published under AGPL too.** The FAQ adds that the AGPL
  can't grant more rights than the training data allows. The training data is
  permissive or public domain: DeepScoresV2 (CC BY 4.0), OpenScore Lieder and
  StringQuartets (CC0), GrandStaff (MIT on Hugging Face, no licence at the
  original source), PDMX public-domain subset (CC BY 4.0).
- **What that means for Cantor (not legal advice):**
  1. **Accept AGPL.** Cantor's source becomes public under AGPL. There is a
     known App Store conflict with GPL-family licences (Apple's terms add
     restrictions). Some AGPL apps ship there anyway because the copyright
     holders allow it. Here the holders are homr's contributors, not us.
  2. **Use only the models and write our own pipeline.** The weights are
     AGPL-labelled, so this needs a lawyer's view on whether weights carry
     the licence.
  3. **Train our own models** on the same permissive datasets with homr's
     published architecture ideas (the architecture is from
     Polyphonic-TrOMR). This is clean, but costs weeks plus GPU time.
  4. **Keep note recognition off the device**: Audiveris (also AGPL) on the
     desktop, or the parked server, as today.

## Reproducing

Engine setup and runs used throwaway venvs (`uv venv`), not committed:

```
uv pip install homr          # 0.7.1, downloads models on first run
uv pip install oemer "numpy<2" "opencv-python<5"
homr page.png                # writes page.musicxml next to the image
python -c "...force CPUExecutionProvider...; from oemer.ete import main; main()" -o . page.png
python tools/omr/compare_engines.py --run-dir RUN --ref-dir scores/out \
    --coverage-dir scores/out/pdf [--first-page-only] --engines homr oemer
```

`RUN/<engine>/<stem>-p<N>/` holds each page's MusicXML. `RUN/<engine>/<stem>-p<N>.time`
holds the `/usr/bin/time -l` output plus an `exit=<code>` line.
