# Cantor roadmap in phases

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
| UI harness without private scores: generate the PDF and measure map from a public fixture (`fixtures/musicxml/satb_chorale_24.musicxml`), so outside contributors and forks can run it | `SCORE=` defaults to the public fixture; CI passes with no `scores/` folder |
| Keep homr code out of the client | Licence check (homr is AGPL; it would need an explicit review entry) |

Who: you. Cost: nothing.

## Phase 1: free features that the paid editions build on (no firm)

These ship in the free AGPL client and make the paid services worth having
later.

| Work | Exit check |
|---|---|
| Account-free sharing between members: `.cantorset` with practice sheets and instructions (a text page per piece, with a voice part per member) | A director sends a set by AirDrop or Files; a member opens it and sees the instructions and their part highlighted (UI scenario plus a device test) |
| "Instructions" page per score and setlist (text, links, rehearsal marks) | Saved, synced through folder sync, shown in the reader |
| Practice tracks per part exported with the set | Opening the set offers "practise my part" without a re-render |
| Band groundwork: chord symbols in the notation view, lead-sheet mode, transposition per instrument | PRD entries written first (no BAND requirements exist yet), then tests |
| Note recognition on a desktop (Audiveris, as today), cleanup of the `cantor-omr` CLI so a choir director can run it | A non-developer turns a PDF into a playable score with one command, documented |

Who: you. Cost: nothing.

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
- Whether homr's model weights carry the AGPL. This decides on-device note
  recognition.
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
| On-device note recognition (only if phase L clears homr's weights, or own models are trained) | The same accuracy gate as the server, on device |

## What is held back right now

- Taking any payment, including donations to the firm. (Personal sponsorship
  of an open-source project is a separate tax question; ask Skatteverket.)
- Running the server for anyone but yourself.
- Signing hosting or Apple agreements in a business name.
- Publishing to the App Store under a seller name.
- Collecting members' practice data or voice recordings.
