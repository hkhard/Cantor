# ChoirBird roadmap in phases

ChoirBird was called Cantor until 2026-11-23. The paid editions are planned
as sister apps: BandBird (band) and ConductorBird (conductor), next to
ChoirBird (choir).

Decided 2026-11-23:

- The Cantor client is AGPL-3.0, with an app store permission
  (`LICENSE-EXCEPTIONS.md`).
- Paid editions (Cantor Choir, Cantor Band, Cantor Conductor) are sold later
  by a business, at first a Swedish sole trader (**enskild firma**).
- The firm is **not registered yet**.

Background: `docs/monetization-feasibility.md`.

Rule for this roadmap:

- Work that **needs the firm** is held back. That means taking money, signing
  contracts, processing other people's data as a service, paid cloud
  accounts, or publishing in the App Store under a business name.
- Everything else can be built now, in the open, as part of the AGPL client
  and as server code. Code written now still counts later.

Each phase lists its exit check, so "done" means something you can observe.

## Phase 0: open client, no firm needed (now)

| Work | Exit check |
|---|---|
| `LICENSE` (AGPL-3.0), `LICENSE-EXCEPTIONS.md` (app store permission), `CONTRIBUTING.md` (CLA v1), licence fields | Files present; `Cargo.toml` / `package.json` say `AGPL-3.0-only` (done 2026-11-23) |
| Dependency licence allow-list in CI, `THIRD_PARTY_NOTICES.md` | `tools/licenses.py` passes; fails on an LGPL crate (tested); CI fails if the notices are stale (done 2026-11-23) |
| About shows the licence and where the source is | UI scenario `about licence` (done 2026-11-23) |
| **Make the repository public** (needed before anyone outside your own devices gets a build: AGPL 6(d)) | Before step: check history for private scores and secrets (`scores/`, `resources/scores/` are ignored; verify with `git log --all -- '*.pdf' '*.mxl' '*.musicxml'`), then flip `hkhard/Cantor` to public, or push a clean copy |
| Tag each build given to others; the About link points at that tag | Build script writes the tag into About |
| UI harness without private scores, so outside contributors and forks can run it | `run.sh` uses `fixtures/scores/mozart-lacrymosa` (MusicXML, PDF, measure map) and never reads `scores/`; CI no longer links private scores; 35 scenarios and 0 a11y findings (done 2026-11-23) |
| Clear or remove *The Anchor Song* (the song's copyright is Björk's; only the arrangement is ours) before the repository is public | Removed 2026-11-25: git-ignored, local only, scrubbed from the history on GitHub; its tests skip when absent |
| homr in the client (decided 2026-11-23: AGPL accepted, on-device note recognition) | `crates/homr` is AGPL-3.0-only without our app store permission and credits homr and its training data. Decided 2026-11-24: App Store builds may contain it once the repository is public and About links the exact source (homr's `LICENSE_FAQ.md`); the app stays free, the backend earns. Residual risk accepted and written down in `crates/homr/README.md` (the FAQ does not mention app stores; fallback: ship without the `homr` feature) |

Who: you. Cost: nothing.

## Phase P: the finished iPad app (now, first priority; no firm)

Decided 2026-11-23: before new features, make Cantor a finished, dependable
iPad app. "Finished" means a choir member could use it daily without help,
and it is ready to submit the day publishing is decided. Submitting it is
not part of this phase (see "held back").

| Work | Exit check |
|---|---|
| **Device check of everything marked "not verified"** in plan section 7: combine MXL+PDF, PDF cursor, voice sets by ear, real share sheet, Apple Pencil, lock-screen controls, keyboard and pedal, two-device sync, drag feel | Each item ticked by the product owner on the iPad mini, or turned into a bug with a test. **2026-11-25, product owner:** real share sheet OK, Apple Pencil OK, Getting started (first run) OK. Pedal: product owner comes back on it. Later the same evening (TestFlight 0.3.3): lock-screen controls OK, voice sets by ear OK, combine MXL+PDF with the cursor on the PDF OK. Still open: pedal (waiting for the pedal); two-device sync and drag feel are not blockers for 1.0 (bugs found later are fixed in an update) |
| **First run:** an empty library explains what to do (import, AirDrop, Files), Lacrymosa is there to try, a short hint for tap-to-play and the mixer | UI scenario from a fresh install; product owner check. **Done in the harness (2026-11-23); device and product owner check open** |
| **Robustness** (in progress: panic guard, corrupt-file tests per importer, atomic writes done 2026-11-23, NFR-13; device test open)**:** no panic crosses into the app (every command returns an error the user can read), damaged or unsupported files give a clear message, low-memory and backgrounding do not lose work | Fuzz/corrupt-file tests per importer; panic guard test; device test: background, kill, reopen. **Device test OK (product owner, 2026-11-25, 0.3.3):** play, background, force-quit, reopen: annotations and settings kept |
| **Performance on the iPad mini:** 60 fps for page turns, scrolling the library and the mixer; cold start and score open times logged | `.perf.log` numbers recorded in plan section 7, within NFR-01/03. **Measured 2026-11-25 (A17 Pro, 3 cold launches):** library and mixer scroll 60 fps with 0 dropped frames; page turns (tap and curl) 59.5 fps with one late frame per turn; open to first page 0.85-0.94 s; launch to library 0.33 s. Follow-up done the same evening: pages are drawn ahead, so the curl turn runs at 60 fps with 0 dropped frames and tap turns drop 1-2 frames in 8 (was one per turn); the curl shows the next page under the lifted sheet (no blank flash). **Product owner, 2026-11-25:** TestFlight 0.3.3 (179) installed on the iPad mini, page turns "work great" |
| **Accessibility on the device:** VoiceOver through library, reader, mixer; larger text; reduced motion | Product owner or VoiceOver walkthrough notes; axe stays at 0. VoiceOver walkthrough: waiting for the product owner's confirmation |
| **Swedish and English complete** | `nfr_10_sv_en_localisation_complete` passes; no hard-coded strings in the UI. **Passes (2026-11-23), in CI** |
| **Long-session soak:** 2 hours of playback without drift or memory growth | `nfr_02_no_drift_gt_5ms`; memory probe in `.perf.log`. **Done 2026-11-25 on the iPad mini (A17 Pro):** 120 min, 40 plays of Lacrymosa on the speaker; audio clock vs wall clock 1.1 ms apart after 2 h (0.15 ppm), no trend; memory 55 MB throughout (see plan section 7) |
| **Release build:** version and build number from a git tag, shown in About with the source link; Release (not debug) configuration; final app icon and launch screen; `PrivacyInfo.xcprivacy` (no tracking, no data collected) | `tools/ipad.sh release` builds an archive; About shows the tag. **1.0.0 (build 195, tag `v1.0.0`) built and uploaded 2026-11-26** |
| **Store-ready material (not submitted):** description sv/en, screenshots of the iPad mini and 13", privacy statement, support page | Files in a new store folder under docs; submission waits for the decision below. **2026-11-26:** listing sv/en and 36 current screenshots uploaded, build 1.0.0 attached to App Store version 1.0.0, review contact, copyright, age 4+ and price in place; Submit for Review is the product owner's step |

Who: you plus agents. Cost: nothing.

## Phase 1: free features that the paid editions build on (no firm)

These ship in the free AGPL client and make the paid services worth having
later. Sharing-with-instructions and Band groundwork were moved to phase 5
(2026-11-23) so phase P comes first.

| Work | Exit check |
|---|---|
| Note recognition on a desktop (Audiveris, as today), cleanup of the `cantor-omr` CLI so a choir director can run it | A non-developer turns a PDF into a playable score with one command, documented. **Done 2026-11-23 except the user test:** `cantor-omr convert … --set NAME` and `cantor-omr doctor`, guide `docs/guides/pdf-to-cantor.md`; Lacrymosa PDF gives 30/30 measures and a set that imports. Not yet tried by a director or on an iPad. |

Who: you. Cost: nothing.

## Phase A: Android (started 2026-11-26; no firm)

Decided 2026-11-26 (product owner): the same app on Android phones and
tablets, package `se.nikielhard.choirbird`, Android 8 and later (minSdk 26,
needed for AAudio). homr comes along, with its models in a Play asset pack.
No OCR. Published on Google Play under the product owner's own account,
free like the iOS app. Fixes to iOS 1.0.x come first.

| Work | Exit check |
|---|---|
| **Project and first run on the emulator** | `gen/android` builds an APK; library, Lacrymosa, PDF, playback and cursor work on the Pixel Tablet emulator (Android 16, WebView 133). **Done 2026-11-26:** bundled resources come out of the APK once per version (`and_01`); the PDF opens on older WebViews (`and_02`, pdf.js legacy build); ChoirBird launcher icon |
| **Real device** (the product owner's Samsung) | Same checks by hand; NFR-01 latency ≤ 20 ms and page-turn fps measured and written in plan section 7. **2026-11-26, Galaxy S10 (Android 12, WebView 147):** library, Lacrymosa PDF, playback and cursor work; audio buffer 2 ms (96 frames) after turning on AAudio low-latency mode, output delay 25 ms (was 215 ms on the deep-buffer mixer). Page-turn fps not measured yet |
| **Files:** open-with for `.mxl`, `.musicxml`, `.pdf`, `.4sc`, `.cantorset`, `.cantorbackup`; a sync folder through the Storage Access Framework; share sheet; backups | Each one done by hand on the device, plus a Rust test per parser path |
| **Background playback** (PLAY-12): MediaSession with lock-screen controls | Plays on with the screen off; play/pause from the lock screen |
| **Android back button, insets, TalkBack** | Back closes dialogs, then the reader; nothing under the status or navigation bar; TalkBack walkthrough of library, reader, mixer |
| **homr on Android:** ONNX Runtime for Android, PDF pages via `PdfRenderer`, models as a Play asset pack | A PDF becomes a playable score on the device. **2026-11-26, Galaxy S10:** works in a development build (models inside the APK, ONNX Runtime 1.31.1 from Microsoft's AAR, CPU): the product owner's one-page camera scan became 10 measures (voice and piano) in 65 s, then played with the cursor on the scan. Still to do: the Play asset pack for the models, a release-size APK/AAB, more pages and scores |
| **Practice-track export** | WAV first, then AAC through MediaCodec |
| **Scan to PDF (LIB-12) on Android** with Google's ML Kit document scanner. **Decided 2026-11-26 (product owner): option (a)**, ML Kit accepted as a reviewed exception (ML Kit Terms of Service, proprietary but free, runs on the device in Google Play services; recorded in `tools/licenses.py` REVIEWED and `THIRD_PARTY_NOTICES.md`). Option (b), an open scanner in Rust, stays possible later. **2026-11-26:** the scanner opens from Import on the S10, and cancelling returns to ChoirBird with nothing added | A scan of a real sheet on the S10 becomes a PDF score |
| **Release:** upload key, AAB, Play internal testing, sv/en listing, Data safety form (no data collected) | Internal testers install from Play |

## Version 1.1 or later: noted for redesign

Kept as they are for 1.0 (decided by the product owner 2026-11-26), to be
updated and optimised in 1.1 or later:

| Work | Exit check |
|---|---|
| **iPhone library header and landscape layout.** Today (0.9 and later): on iPhone in portrait the layout and About pills share a row with the colour picker on the row below; in landscape the mark and title shrink, the subtitle hides and all three pills sit on one row; the library pads by the safe areas on every side so nothing sits under the notch / Dynamic Island. The product owner does not like how it is done (screenshot from an iPhone 16 in landscape, 2026-11-26). | A redesigned iPhone home screen (portrait and landscape) agreed with the product owner from a mock-up first; checked on the iPhone 16 in both orientations; harness step `lib_11 the layout, colour and About pills…` updated to the new design; a11y 0 at 200% text |
| **Columns layout polish on iPad** (drag feel, hold time before a list opens, slide speed, card density) once there is real use | Product owner and testers comfortable with it on device; numbers in plan section 7 |

## Phase 2: server code, written and run privately (no firm, no customers)

The org server can be **developed and tested** before the firm exists. It is
not offered to anyone else until phase 4.

- **The server's licence is your choice.** It is a separate program, so it
  can be closed. You hold all of its copyright.
- **Exception: an AGPL engine on the server** (Audiveris, homr). Its source
  must be offered to users of that service.

| Work | Exit check |
|---|---|
| Org model: organisation, members, roles (Conductor, section leader, member) | API tests |
| Push setlists, practice sheets and instructions to members (reusing `.cantorset` and the `sync` crate's merge rules) | Two simulated devices plus a server converge to the same library hash (same as `data_02`) |
| Client sign-in, optional; offline use unchanged | UI scenario: signed out, everything works as today |
| Server note recognition (Audiveris + homr in a container, AGPL source published) | PDF in, MusicXML and measure map out, on the 13 sample scores |
| Singing renders on a GPU, with a cache per score, part and voice | One part rendered and cached; a second request is served from the cache |

Who: you. Cost: your own test hosting only (a free tier or your own Mac).

## Phase F: the firm (held back until you decide)

Steps outside the code, in order:

1. Register the **enskild firma** with Bolagsverket and Skatteverket
   (F-skatt, VAT if the turnover needs it).
2. **Transfer the copyright** in Cantor (client and server) from you to the
   firm.
   - A short written assignment. For an enskild firma, you and the firm are
     legally the same person, so this mainly documents ownership.
   - It matters more if you later move to an aktiebolag.
3. Update `CONTRIBUTING.md`: "Maintainer" becomes the firm (the CLA already
   allows this).
4. **Apple Developer Program** in the firm's name.
   - An individual account shows your personal name as the seller.
   - An organisation account needs a legal entity with a D-U-N-S number.
   - In Sweden an enskild firma usually counts as an individual. Check
     whether an organisation account is possible, or whether an aktiebolag is
     needed for the App Store seller name.
5. **Lawyer review** (phase L below).

Nothing in phases 3 and 4 starts before steps 1–3.

## Phase L: legal review (needs a budget, best done with the firm)

One review covering:

- The section 7 app store wording and the CLA.
- Verovio (LGPL-3.0) in a statically linked app.
- homr in App Store builds (decided 2026-11-24 to ship once the source is
  public; review the residual app store risk written in
  `crates/homr/README.md`) and whether the weights' training data licences
  need more than the attribution in About.
- Apple guideline 3.1.3(c) for choir seats.
- Terms of service, privacy policy and a DPA template for organisations
  (GDPR).

## Phase 3: paid services, beta (needs the firm)

| Work | Needs from the firm | Exit check |
|---|---|---|
| Hosting in the EU (Cloudflare or similar) on the firm's account | Account, card, DPA with the host | Server reachable; data stays in the EU |
| Conductor beta with 1–3 choirs (free, but under terms) | Terms of service, privacy policy | A real choir uses push setlists for a concert cycle |
| Server note recognition for beta choirs | Same | Accuracy and failure rate measured on their scores |
| Singing renders: a listening test with choir members | Same, plus consent for recordings | A pass mark agreed in advance (e.g. most members would use it) |

## Phase 4: sales (needs the firm and phase L)

| Work | Exit check |
|---|---|
| Conductor as an organisation subscription with member seats (web checkout; App Review consulted on 3.1.3(c) first) | First paying organisation |
| Choir and Band as individual subscriptions (in-app purchase plus web at the same price) | IAP approved in App Review |
| Prepared S/A/T/B voices as content | Recorded with consent under the firm's licence |
| On-device note recognition: being built now (homr port, `crates/homr`); public store release waits for the homr permission question above | The same accuracy gate as the server, on device |

## Phase 5: moved here from phase 1 (2026-11-23, after phase P)

Moved here on 2026-11-23 so the iPad app is finished first. Core code for
BAND-01..03 and SET-03..05 was already committed by then (tests pass), with
a first UI; product owner review and device checks wait for this phase.
Same exit checks as before.

| Work | Exit check |
|---|---|
| Account-free sharing between members: `.cantorset` with practice sheets and instructions (a text page per piece, with a voice part per member) | A director sends a set by AirDrop or Files; a member opens it and sees the instructions and their part highlighted (UI scenario plus a device test) |
| "Instructions" page per score and setlist (text, links, rehearsal marks such as "t. 12" that jump to the bar), synced and in backups | Saved, synced through folder sync, shown in the reader |
| Practice tracks per part exported with the set | Opening the set offers "practise my part" without a re-render |
| Band groundwork: PRD BAND requirements first, then chord symbols (`<harmony>`) parsed, transposed with key-aware spelling and shown; per-player instrument transposition (C, B♭, E♭, F) with sound at concert pitch; optional chord comping playback | PRD entries written first (no BAND requirements exist yet), then tests |

## What is held back right now

- Taking any payment, including donations to the firm. (Personal sponsorship
  of an open-source project is a separate tax question; ask Skatteverket.)
- Running the server for anyone but yourself.
- Signing hosting or Apple agreements in a business name.
- Publishing to the App Store under a business seller name. (Decided 2026-11-25: ChoirBird 1.0, free, may be published under the product owner's personal developer account, seller shown as his name; the legal review of phase L is not a blocker for the free 1.0.)
- Collecting members' practice data or voice recordings.
