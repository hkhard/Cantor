# Monetisation feasibility: AGPL Cantor client plus paid Choir / Band / Conductor

Date: 2026-11-23. **Not legal advice.** Every licence point below needs a
lawyer before money changes hands.

## The decision being tested

- **The client stays open.** The Cantor client app is licensed AGPL-3.0.
- **A separate company sells three editions on top of it:**
  - Cantor Choir.
  - Cantor Band.
  - Cantor Conductor.
- **What the editions sell:**
  1. Central management: a director arranges setlists, practice sheets and
     instructions, and pushes them to members.
  2. Note and text recognition on the device.
  3. Sung lyrics with SoulX-Singer and prepared voices.

## Short answer

**Feasible, if each paid feature is built as a service or as content, and
never as locked code inside the app.** Three of the four pillars hold up well.
On-device note recognition in an App Store build is the weak one, because of
homr's licence.

| Upsell | Feasible? | Why |
|---|---|---|
| Central setlists, practice sheets, instructions (Conductor pushes to Choir/Band) | **Yes, best fit** | It is a server service. The server can be closed source because it is a separate program the AGPL client talks to. Users cannot copy it by rebuilding the app. Most of the groundwork exists (`.cantorset` bundles, the `sync` crate with merge rules, setlist order) |
| On-device text recognition (titles, credits) | Yes, but it cannot be paid on its own | Apple Vision is built into iOS, and the code is AGPL in the client. Anyone can rebuild it unlocked. Sell it as part of the bundle, not as the reason to pay |
| On-device note recognition (homr) | **Technically yes, legally blocked on the App Store** | The spike shows it fits the iPad (12–16 s per page, ≤ 1.2 GB). But homr's code is AGPL, and only its copyright holders can grant an App Store exception. homr says that "nobody can relicense the code". Paths are below |
| Sung lyrics (SoulX-Singer, prepared voices) | Yes, as a service and content; on the device is unproven | SoulX is Apache-2.0 (code and weights), so licensing is fine. The model is 2.8 GB fp32 / ~1.4 GB fp16 and renders at RTF 5.5 on an M2, and singing was parked as "barely passable" with no Swedish. Rendering on a server, plus prepared voices sold as content, works today. On-device rendering needs a Core ML spike |

## 1. AGPL and the App Store

- **The conflict.** Apple's App Store terms add usage rules (limited devices,
  no redistribution, FairPlay DRM). The FSF holds that those are "further
  restrictions" the GPL-family licences forbid. VLC was pulled from the App
  Store in 2011 over this.
- **The usual fix.** Projects add an extra permission under AGPL section 7
  that allows App Store distribution. Signal does this, and Nextcloud iOS has
  an "App Store exception".
- **The catch.** Only the copyright holders of the covered code can grant that
  permission.

What this means for Cantor:

1. **Cantor's own code:** we hold all the copyright today (single author, no
   outside contributions). The company can grant the section 7 App Store
   permission. Do it **now**, while there is one copyright holder.
2. **From the first outside contribution on:** require a CLA (or copyright
   assignment) to the company. Without it the company cannot:
   - keep the App Store permission valid for new code,
   - dual-license,
   - or move code into a closed server later.
3. **Third-party code in the client:**

   | Component | Licence | Fits AGPL + App Store permission? |
   |---|---|---|
   | Rust crates (≈ 470) | MIT / Apache-2.0 / Zlib / Unicode / ISC | Yes |
   | symphonia, cssparser and others | MPL-2.0 | Yes (file-level copyleft; their source stays public anyway) |
   | Verovio | LGPL-3.0 | Yes. LGPL is GPL-family, but static linking into an app whose whole source is public meets its relinking terms. Check with a lawyer |
   | pdf.js | Apache-2.0 | Yes |
   | Apple Vision (text recognition) | System framework | Yes (system library exception) |
   | ONNX Runtime | MIT | Yes |
   | GeneralUser GS soundfont | Free for commercial use; author cannot vouch for every sample's origin | Acceptable risk, noted in its licence |
   | **homr** (code and weights) | **AGPL-3.0, no App Store permission, no commercial licence** | **No, for App Store builds** |

So the plain AGPL client with our own section 7 permission can ship on the
App Store. **Bringing homr's code into it brings back the conflict for that
part**, and we cannot fix it ourselves.

### Ways to get on-device note recognition anyway

1. **Port only the models, write our own pipeline.**
   - Our own code: staff detection, dewarping, MusicXML writer.
   - Only homr's ONNX weights are used. They are AGPL-labelled, but trained on
     CC0 / CC BY / MIT data.
   - Whether weights carry the code licence is an open legal question, as
     homr's own FAQ says. **A lawyer must sign this off.** Cheapest option if
     the answer is yes.
2. **Train our own weights** with the same published architecture
   (Polyphonic-TrOMR plus the oemer-style U-Net) on the same permissive
   datasets.
   - Clean licence.
   - Cost: weeks of work and GPU time, plus a benchmark showing we are not
     worse.
3. **Distribute outside the App Store** in the EU (Web Distribution or an
   alternative marketplace).
   - Apple's App Store usage rules are not part of those channels.
   - But these builds are still notarised under Apple's terms, there is a 5%
     Core Technology Commission, and it is EU only.
4. **Keep note recognition as a service.** Audiveris and homr run on our
   server; both are AGPL there, and we publish that source, which the AGPL
   network clause requires anyway.
   - It is a paid service with no App Store conflict.
   - Cost: servers (the parked Cloudflare Containers plan, roughly 1 cent per
     score, not measured).

**Recommended order:** 4 now (server, paid). Then ask a lawyer about 1. If
the answer is no, 2.

## 2. What can actually be sold when the client is AGPL

Anyone may rebuild the AGPL client with the paywall removed. So **a feature
flag inside the app is not a product.** What is a product:

- **Services** run by the company. The server code can stay closed, because a
  separate program talking over a network is not a derivative of the client.
  - Organisation accounts and roles (Conductor / section leader / member).
  - Central setlists, practice sheets, instructions and annotations pushed
    to members.
  - Server-side note recognition (Audiveris + homr) for PDFs.
  - Server-side singing renders (SoulX on a GPU) with a render cache.
  - Hosting, backup and history.
- **Content** under its own licence. The AGPL covers code, not data the app
  downloads.
  - Prepared voice prompts per S/A/T/B, recorded under our licence
    (WP-5.6b).
  - Rendered practice tracks.
  - Curated measure maps.
- **Convenience.** The App Store build, updates and support. Many open-source
  apps sell exactly this.

The client can show and use all of these. It just needs an account.

## 3. Apple payment rules for the three editions

From the App Review Guidelines (read 2026-11-23) and fee reports from
2026-11:

- **3.1.1:** unlocking features or content in the app needs in-app purchase.
  - Choir and Band sold to **individuals** must offer IAP: 30% the first
    year, 15% from the second year of a subscription or in the Small Business
    Program.
- **3.1.3(c) Enterprise Services:** apps sold directly to organisations or
  groups "for their employees or students" may let those users in without
  IAP.
  - A choir or band buying seats for its members is close to this, but
    members are neither employees nor students. **Ask App Review before
    relying on it.** Conductor as an organisation licence is the strongest
    case.
- **3.1.3(b) Multiplatform:** web purchases may unlock the app, as long as
  the same items are also sold as IAP.
- **External purchase links:**
  - **United States:** links and buttons are allowed with no entitlement, at
    0% commission for now (pending a court ruling on a rate).
  - **EU** (incl. Sweden): needs an entitlement and pays a 15% store-services
    commission (10% reduced).
  - **Elsewhere:** no in-app steering.

Pricing that works with these rules:

- **Conductor** is an organisation subscription bought on the web, with seats
  for members. Members sign in; no IAP needed under 3.1.3(c) if App Review
  agrees.
- **Choir and Band** as individual subscriptions: IAP plus web, at the same
  price.
- **The free client** stays fully useful offline: reading, playback,
  annotations, setlists, folder sync.

## 4. Engineering feasibility per edition

| Edition | Needs | Exists today | Missing |
|---|---|---|---|
| **Choir** (singer) | Receive setlists and practice sheets, practice per part, sung guide part | Reader, part playback, practice mixes, M4A export, setlists, `.cantorset`, folder sync with merge rules, annotations | Account sign-in, pull from the org server, render-cache download |
| **Band** (instrumentalist) | Chord charts, transposition per instrument, click and count-in, lead sheets | Transpose and concert pitch, `<transpose>` parsing, metronome, backing tracks | Chord symbols and lead-sheet view, instrument-specific parts (not in the PRD yet), drum/click maps. **Largest new scope** |
| **Conductor** (director) | Arrange setlists for a group, assign parts, write instructions, see who practised | Setlist editing, annotations with layers, sync engine (DATA-02) | Org server (accounts, roles, push), instruction pages, practice reporting (privacy: members opt in) |

**The server, which is new:**

- **Accounts and organisations.**
- **Sync of `library.json` and annotations per org.** The `sync` crate's
  per-device logs, last-writer-wins and observed-remove merge rules can run on
  the server as well.
- **File storage:** PDFs, MusicXML, measure maps, practice tracks.
- **Job queue for note recognition and singing renders.**
- **Hosting.** Cloudflare Workers + R2 + D1 for the API and files, plus
  Containers or a GPU host for Audiveris, homr and SoulX.
- **Rough size:** 6–10 weeks for a first org server with the Conductor push
  flow. An estimate, not measured.

## 5. Singing (SoulX-Singer)

- **Licence:** Apache-2.0 for code and weights. This allows commercial use,
  on our own server or inside the AGPL client.
- **Quality:** parked on 2026-11-22 as "barely passable".
  - English only, with no Swedish phonemes.
  - Measured median pitch error 19 cents.
  - The product owner's ears decide.
- **On the iPad:** 704 M parameters, 2.8 GB fp32, ~1.4 GB as Core ML fp16.
  RTF 5.5 on an M2 GPU: 38 s to render 7 s of singing.
  - The spike doc judged it "not feasible as shipped" on iPhone or iPad.
  - A Core ML conversion spike is needed before any on-device promise.
- **As a paid service:** render on a GPU server, cache per score, part and
  voice, then download the M4A. This fits the Choir and Conductor editions
  today, with no on-device constraint.
- **Prepared voices:** record our own S/A/T/B prompts with consent and our own
  licence, and sell them as content (VOX-09).

## 6. Risks

| Risk | Effect | Mitigation |
|---|---|---|
| An outside contributor's AGPL code without the App Store permission | Blocks App Store releases (the VLC case) | CLA from the first contribution; the section 7 permission is in `LICENSE` before going public |
| homr licence | No on-device note recognition in App Store builds | Server first; lawyer on weights; own training as the fallback |
| A rebuilt client with the paywall removed | Lost sales of in-app gates | Sell services and content, not gates |
| App Review rejects 3.1.3(c) for choirs | Org seats need IAP | Offer IAP seats too; ask App Review early (pre-submission contact) |
| Singing quality | The upsell does not convince | Keep it optional; measure with choir members before charging |
| Server cost (GPU for singing) | Margin | Render cache per score, part and voice; renders only on request; price per render or in tiers |
| GDPR (practice reporting, voices) | Legal exposure | Opt-in, data in the EU, a DPA for organisations, no voice cloning of members without consent |

## 7. Plan

1. **Now:**
   - Add `LICENSE` (AGPL-3.0) with a section 7 App Store permission for
     Cantor's own code, and a CLA in `CONTRIBUTING.md`.
   - Update the `license` field in `Cargo.toml` and the `package.json` files.
   - Add a CI check that fails on dependency licences outside an allow-list.
   - Do not commit homr code to the client.
2. **Lawyer** (one review): the section 7 wording, the CLA, Verovio's LGPL in
   a static app, whether homr's weights carry the AGPL, and the 3.1.3(c)
   reading.
3. **Company:**
   - The entity owns the copyright, through assignment from the author and
     the CLA.
   - It sells the three editions as services.
4. **Server MVP (Conductor first):**
   - Org accounts, roles, and pushing setlists and practice sheets (reusing
     `.cantorset` and the `sync` crate).
   - Client sign-in, with offline use unchanged.
5. **Server note recognition** (the parked plan, revived): Audiveris + homr
   in containers. Publish the server's recognition-container source, which
   the AGPL requires.
6. **Singing service:**
   - GPU render queue for SoulX, a cache, and prepared S/A/T/B voices.
   - A listening test with choir members before charging.
7. **Band edition:** scope chord charts and instrument parts in the PRD first.
   It is the largest new feature set.
8. **On-device note recognition:** only after the lawyer's answer on weights
   (path 1) or our own trained models (path 2).

Sources: App Review Guidelines 3.1.1, 3.1.1(a), 3.1.3(b), 3.1.3(c) (read
2026-11-23); homr `LICENSE_FAQ.md`; SoulX-Singer model card (Apache-2.0);
2026 reports on App Store fees (US link-outs 0% for now, EU 15% link-out);
public GitHub discussions of AGPL section 7 App Store permissions (Signal,
Nextcloud iOS, VLC history); `docs/spikes/singing-engines.md`;
`docs/omr-engine-comparison.md`.
