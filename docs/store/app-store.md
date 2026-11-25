# ChoirBird on the App Store

Everything App Store Connect asks for in a full release, in English (primary
language) and Swedish. Character limits are App Store Connect's; the counts
are checked by `tools/store/check_text.py` (run it after any edit).
Screenshots: see "Screenshots" at the end. No app preview video for now.

## App information (once per app)

| Field | Value |
|---|---|
| Name | ChoirBird |
| Bundle ID | se.nikielhard.choirbird |
| SKU | choirbird-ios |
| Primary language | English (U.K.) |
| Primary category | Music |
| Secondary category | Education |
| Content rights | Contains no third-party content (the bundled Lacrymosa is Mozart, public domain; users bring their own scores) |
| Age rating | 4+ (no objectionable content; every answer "None"/"No"; no unrestricted web access; no user-generated content shared with others) |
| Price | Free, all countries (no In-App Purchases) |
| Copyright | 2026 (fill in: the legal name of the copyright holder) |
| Licence agreement | Apple's standard EULA. The source code is AGPL-3.0 with the app store permission in `LICENSE-EXCEPTIONS.md`; this does not need a custom EULA |
| Support URL | https://nikielhard.se/en/choirbird |
| Marketing URL | https://nikielhard.se/en/choirbird |
| Privacy Policy URL | https://nikielhard.se/en/choirbird/privacy |
| Swedish Privacy Policy URL | https://nikielhard.se/se/choirbird/privacy |
| App icon (1024 px) | Taken from the build (`ChoirBird.appiconset/AppIcon-1024.png`, with dark and tinted variants); nothing to upload |

All four URLs above and the source link answered HTTP 200 on 2026-11-25.

## App Privacy ("nutrition label")

- **Data collection:** "No, we do not collect data from this app."
- Reason: ChoirBird makes no network calls (enforced by the UI harness),
  has no accounts, analytics, ads or tracking. `PrivacyInfo.xcprivacy`
  declares no collected data types and no tracking; it lists file
  timestamps (C617.1) as the only required-reason API.
- Note recognition and title reading run on the device.

## Accessibility Nutrition Labels

Claim only what was verified (see `docs/IMPLEMENTATION_PLAN.md`):

| Label | Claim? | Evidence |
|---|---|---|
| Dark Interface | Yes | Dark skin; music pages light grey on black |
| Sufficient Contrast | Yes | Skins ≥ 4.5:1 text, ≥ 7:1 music page (UI harness `skins`, `a11y`) |
| Reduced Motion | Yes | `prefers-reduced-motion` honoured (harness step) |
| Larger Text | Not yet | 200% text checked in WebKit only, not with iOS Dynamic Type on a device |
| VoiceOver | Not yet | Controls are named (axe 0 findings) but not tried with VoiceOver on a device |
| Voice Control | Not yet | Not tried |
| Differentiate Without Color | Not yet | Voice colours carry names too, but not reviewed |
| Captions, Audio Descriptions | Not applicable | No video |

## Version information (per version)

### English (U.K.)

**Subtitle** (30):

```
Hear your part, sing it better
```

**Promotional text** (170, can change without a new version):

```
Practise your part with the score you already have. ChoirBird plays every voice on its own line, follows the music bar by bar, and works offline. Free.
```

**Keywords** (100, comma separated, no spaces after commas; do not repeat
words from the name or subtitle):

```
choir,rehearsal,practice,sheet music,score,satb,musicxml,pdf,forscore,singing,learn,alto,tenor,bass
```

**Description** (4000):

```
ChoirBird is a practice app for choir singers. Open the score your conductor handed out and ChoirBird plays every voice on its own line, so you can learn your part at home, on the bus or on the way to rehearsal.

YOUR OWN SCORE
• The original PDF, exactly as printed. Page and bar numbers match everyone else's.
• Or the notes re-engraved from MusicXML, with transposition and parts you can hide.
• The bar being played is marked on the page, and the pages turn by themselves.

EVERY VOICE ON ITS OWN
• Soprano, alto, tenor, bass and piano each get a line with its own instrument.
• My part: your voice louder, the others softer. Without me: sing along with everyone but you.
• Volume, mute and solo for each voice, and mixes you can save per piece.

MADE FOR REHEARSING
• Slow the tempo down without changing the pitch.
• Loop a tricky passage between two bars.
• Click, count-in and a pitch pipe for your first note.
• Tap a bar to start from there.

READ THE NOTES FROM A PDF
• Only have a PDF? ChoirBird can read the notes from it, here on your device, and then play every part. It takes about a minute per page and carries on while you do other things.

MARK YOUR MUSIC
• Pen, highlighter, text and stamps (breaths, dynamics, cues) right on the page, in layers you can switch.

ON STAGE
• Performance mode shows only the music, with a small bar for pages and the pieces of a setlist.
• Turn pages with a tap, a swipe like a book, or a Bluetooth page-turn pedal.
• Light, dark and sepia, for any light.

YOUR LIBRARY
• Import MusicXML, PDF and forScore files (.4sc and .4ss) from Files, AirDrop or "Open in ChoirBird".
• Setlists, search, tags and ratings. Sync through a folder in iCloud Drive or another cloud folder, and back up your whole library.
• Export practice tracks for your part.

PRIVATE AND OFFLINE
ChoirBird works without an internet connection, has no account and collects no data. Your scores stay on your device or in a folder you choose.

Mozart's Lacrymosa is included so you can try everything straight away.

ChoirBird is free software under the GNU AGPL-3.0. The source code of every version is linked in About ChoirBird.

For iPad and iPhone, in English and Swedish.
```

**What's New in This Version** (4000):

```
The first App Store release of ChoirBird.
• Read the notes from a PDF, on your device.
• Repeats, first and second endings, D.S. al Coda and Coda play as written.
• Tied and slurred notes on the same pitch are held as one note.
```

### Swedish

**Subtitle** (30):

```
Hör din stämma, sjung säkrare
```

**Promotional text** (170):

```
Öva din stämma med noterna du redan har. ChoirBird spelar varje stämma för sig, följer musiken takt för takt och fungerar utan nät. Gratis.
```

**Keywords** (100):

```
kör,körapp,övning,repetition,noter,notblad,satb,musicxml,pdf,forscore,sång,sopran,alt,tenor,bas
```

**Description** (4000):

```
ChoirBird är en övningsapp för körsångare. Öppna noterna du fått av dirigenten så spelar ChoirBird varje stämma för sig, så att du kan lära dig din stämma hemma, på bussen eller på väg till repetitionen.

DINA EGNA NOTER
• Original-PDF:en, precis som den är tryckt. Sid- och taktnummer stämmer med alla andras.
• Eller noterna omsatta från MusicXML, med transponering och stämmor som du kan dölja.
• Takten som spelas markeras på sidan, och bladen vänds av sig själva.

VARJE STÄMMA FÖR SIG
• Sopran, alt, tenor, bas och piano får var sin rad med eget instrument.
• Min stämma: din stämma starkare, de andra svagare. Utan mig: sjung med alla utom dig själv.
• Volym, tyst och solo för varje stämma, och mixar som sparas per stycke.

GJORD FÖR ATT ÖVA
• Sänk tempot utan att tonhöjden ändras.
• Slinga ett svårt ställe mellan två takter.
• Klick, inräkning och stämton för din första ton.
• Tryck på en takt för att börja där.

LÄS IN NOTERNA UR EN PDF
• Har du bara en PDF? ChoirBird kan läsa noterna ur den, här på enheten, och sedan spela alla stämmor. Det tar ungefär en minut per sida och fortsätter medan du gör annat.

ANTECKNA I NOTERNA
• Penna, överstrykning, text och stämplar (andningar, dynamik, insatser) direkt på sidan, i lager som du kan byta mellan.

PÅ SCENEN
• Läget Framför visar bara noterna, med en liten list för sidor och styckena i en repertoarlista.
• Vänd blad med ett tryck, ett svep som i en bok eller med en bladvändarpedal.
• Ljust, mörkt och sepia, för alla ljus.

DITT BIBLIOTEK
• Importera MusicXML, PDF och forScore-filer (.4sc och .4ss) från Filer, AirDrop eller "Öppna i ChoirBird".
• Repertoarlistor, sökning, taggar och betyg. Synka via en mapp i iCloud Drive eller annan molnmapp, och säkerhetskopiera hela biblioteket.
• Exportera övningsspår för din stämma.

PRIVAT OCH UTAN NÄT
ChoirBird fungerar utan internet, har inget konto och samlar inte in några data. Dina noter stannar på enheten eller i en mapp som du väljer.

Mozarts Lacrymosa finns med så att du kan prova allt direkt.

ChoirBird är fri programvara under GNU AGPL-3.0. Källkoden för varje version länkas under Om ChoirBird.

För iPad och iPhone, på svenska och engelska.
```

**Nyheter i den här versionen** (4000):

```
ChoirBirds första version på App Store.
• Läs in noterna ur en PDF, direkt på enheten.
• Repriser, volta (1. och 2. gång), D.S. al Coda och Coda spelas som de är skrivna.
• Bundna noter och legatobågar på samma ton hålls ut som en ton.
```

## App Review information

- **Sign-in required:** No.
- **Contact:** choirbird-support@nikielhard.se (name and phone: fill in).
- **Notes for the reviewer:**

```
ChoirBird needs no account and makes no network connections.
To try it: on first start the library shows "Getting started with ChoirBird"; tap "Try Lacrymosa" (Mozart, included), then press Play. Open the mixer (sliders icon) to hear one voice alone: tap a voice name, then "My part".
On-device note recognition: open any PDF score that has no MusicXML and tap "Read the notes" at the bottom. It runs on the device and takes about a minute per page.
Audio keeps playing in the background on purpose (rehearsal with the screen locked), which is why the app declares the audio background mode.
The source code is public (GNU AGPL-3.0): https://github.com/hkhard/Cantor
```

## Export compliance

`ITSAppUsesNonExemptEncryption` = NO is in the build's `Info.plist`, so no
question is asked per build.

## Screenshots

Made by `tools/store/make_screenshots.sh` (real UI in WebKit, public-domain
titles only, framed by `tools/store/compose.py` after the
aso-appstore-screenshots skill: navy background, coral action verb, cream
benefit line, drawn device frame). Headlines live in
`tools/store/headlines.json`. Output (not in git; regenerate any time):
`$OUT/store/<lang>/<slot>/NN-name.png`, 6 per slot, 36 in total.

| App Store Connect slot | Folder | Size (portrait) |
|---|---|---|
| iPhone with Dynamic Island, large display (6.9") | `iphone-69` | 1320 × 2868 |
| iPhone with Dynamic Island, medium display (6.3", **required**) | `iphone-63` | 1206 × 2622 |
| iPad 13" display (**required**, the app runs on iPad) | `ipad-13` | 2064 × 2752 |

Upload `en/` to English (U.K.) and `sv/` to Swedish, in file order:

| # | English | Swedish | Shows |
|---|---|---|---|
| 1 | PRACTISE your part in your own score | ÖVA din stämma i dina egna noter | The PDF with the playing bar marked |
| 2 | HEAR every voice on its own | HÖR varje stämma för sig | Mixer, My part on the alto (iPhone: the alto row pops out) |
| 3 | FOLLOW the music bar by bar | FÖLJ musiken takt för takt | Re-engraved notation with the cursor |
| 4 | MARK breaths, dynamics and cues | MARKERA andningar, dynamik och insatser | Highlighter and an *mf* stamp on the PDF |
| 5 | KEEP your whole repertoire at hand | SAMLA hela repertoaren på ett ställe | Library with setlists |
| 6 | READ in any light, even on stage | LÄS i alla ljus, även på scenen | Dark skin |

App Store Connect accepts PNG or JPEG, RGB without alpha; the script checks
every file's size and mode.

**Not verified:** how the set looks in App Store Connect's own preview and on
a phone in the App Store; the status bar and device frame are drawn, not
taken from a device.
