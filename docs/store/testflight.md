# ChoirBird on TestFlight

For the full App Store release (listing text, privacy answers, screenshots)
see `docs/store/app-store.md`.

Text to paste into App Store Connect › ChoirBird › TestFlight. Swedish
first (the choir is Swedish), English below. Limits: Beta App Description
4,000 characters, "What to Test" 4,000.

## Public link

**Public link:** _not created yet._ Create it in App Store Connect ›
TestFlight › External Testing › (group) › Public Link, then paste it here:

`https://testflight.apple.com/join/XXXXXXXX`

**Before turning on the public link** (it gives the app to people outside
the team):

1. **Source is public (AGPL section 6):** done. `hkhard/Cantor` is public and
   About › Source points at the tag of the build (build 0.3.3 →
   tag `v0.3.3`).
2. ***The Anchor Song* fixture:** removed from GitHub and its history
   (2026-11-25); only Lacrymosa is public.
3. **External testing needs Beta App Review** by Apple for the first build:
   allow about a day.
4. **On-device note recognition (homr):** builds since 0.3.1 contain it. Allowed
   because the source of the exact build is public (decision 2026-11-24, see
   `crates/homr/README.md` for the accepted residual risk). homr, its
   training data and ONNX Runtime are credited in About.

## Uploading a build

1. Build the archive (`HOMR=1 tools/ipad.sh release`, see
   `docs/DEVELOPING.md`), tag the commit `vX.Y.Z` and push the tag.
2. Upload: Xcode › Window › Organizer › Archives › the build ›
   Distribute App › App Store Connect › Upload. (Needs an Apple ID with
   access to team 33ZQ6Q226L under Xcode › Settings › Accounts. Without it
   `xcodebuild -exportArchive` fails with "App Store Connect access for
   33ZQ6Q226L is required".)
3. Export compliance is answered by the build itself
   (`ITSAppUsesNonExemptEncryption` = NO in `Info.plist`), so the build is
   ready for testing once App Store Connect has processed it.
4. Paste "What to Test" for the build from this file (both languages),
   add it to the internal group, then to the external group.

## Beta App Description (svenska)

ChoirBird är en övningsapp för körsångare. Den visar dina noter som original-PDF och spelar varje stämma med ett eget instrument, så att du kan öva din stämma, hemma eller på väg till repetitionen.

Det här kan du göra:
• Läsa noter: originalet (PDF) eller omsatta noter från MusicXML. Sidvändning med tryck, svep (som i en bok) eller pedal. Läget "Tryck flyttar en takt" låter dig gå takt för takt.
• Öva: spela alla stämmor eller bara din egen, gör din stämma starkare eller tysta den ("Utan mig"). Tempo, slinga mellan två takter, klick, inräkning och stämton. Markören följer musiken i PDF:en.
• Framföra: läget Framför visar bara noterna och en liten list för sidor och stycken i en repertoarlista.
• Bibliotek: importera MusicXML, PDF och forScore-filer (.4sc och .4ss) via Filer, AirDrop eller "Öppna i ChoirBird". Repertoarlistor, sökning, taggar, betyg. Synk via en mapp i iCloud Drive eller annan molnmapp, och säkerhetskopior.
• Anteckningar: penna, överstrykning och stämplar direkt på noterna.
• iPad och iPhone, ljust, mörkt och sepia, svenska och engelska.

Mozarts Lacrymosa finns med så att du kan prova direkt.

ChoirBird är fri programvara (GNU AGPL-3.0). Källkoden finns under Om ChoirBird › Licens.

## Beta App Description (English)

ChoirBird is a practice app for choir singers. It shows your scores as the original PDF and plays every part with its own instrument, so you can rehearse your own line at home or on the way to rehearsal.

What you can do:
• Read: the original PDF or re-engraved notation from MusicXML. Turn pages by tapping, swiping (like a book) or with a pedal. "Tap moves one bar" lets you step bar by bar.
• Practise: play every part or only yours, make yours louder or mute it ("Without me"). Tempo, loop between two bars, click, count-in and pitch pipe. The cursor follows the music on the PDF.
• Perform: Performance mode shows only the music and a small bar for pages and the pieces in a setlist.
• Library: import MusicXML, PDF and forScore files (.4sc and .4ss) through Files, AirDrop or "Open in ChoirBird". Setlists, search, tags, ratings. Sync through a folder in iCloud Drive or another cloud folder, and backups.
• Annotate: pen, highlighter and stamps on the music.
• iPad and iPhone, light, dark and sepia, Swedish and English.

Mozart's Lacrymosa is included so you can try it straight away.

ChoirBird is free software (GNU AGPL-3.0). The source is linked in About ChoirBird › Licence.

## What to Test (build 0.3.3)

Svenska:
• Nytt: ChoirBird kan läsa noterna ur en PDF, direkt på enheten. Öppna en PDF som saknar MusicXML och tryck "Läs in noterna" längst ner. Det tar ungefär en minut per sida (första gången längre) och fortsätter medan du gör annat. Spelar stämmorna rätt efteråt? Blev det rätt antal stämmor?
• Nytt: forScore-filer (.4sc) med bara PDF kan läsas in på samma sätt. Behåller stycket sitt namn?
• Repriser, volta (1. och 2. gång), D.S. al Coda och Coda spelas nu som i MuseScore. Lyssna på ett stycke med många repriser: hoppar uppspelningen rätt?
• Nytt i 0.3.2: en båge mellan två noter på samma ton spelas som en enda utdragen ton, både bindebåge och legatobåge (till exempel "da-ah" i slutet av Only You). Bundna noter hålls ut även när två stämmor håller samma ton. Låter det som du sjunger det?
• Nytt i 0.3.3: sidvändningen är jämnare. När du sveper syns nästa sida under bladet som vänds, i stället för en tom sida. Ser det ut som en riktig bok, och blinkar något?
• Fortsätt gärna testa som förut: din stämma, Utan mig, tempo, slinga, bladvändning och läget Framför.
• Berätta vad som var svårt, gick fel eller saknas (skärmbild i TestFlight). Skriv gärna vilket stycke och vilken takt.

English:
• New: ChoirBird can read the notes from a PDF, on the device. Open a PDF that has no MusicXML and tap "Read the notes" at the bottom. It takes about a minute per page (longer the first time) and carries on while you do other things. Do the parts play correctly afterwards? Did you get the right number of parts?
• New: forScore files (.4sc) with only a PDF can be read the same way. Does the piece keep its name?
• Repeats, first and second endings, D.S. al Coda and Coda now play as in MuseScore. Listen to a piece with many repeats: does playback jump to the right places?
• New in 0.3.2: an arc between two notes of the same pitch plays as one held note, whether it is a tie or a slur (for example "da-ah" at the end of Only You). Tied notes are held also when two parts hold the same note. Does it sound the way you sing it?
• New in 0.3.3: smoother page turns. When you swipe, the next page shows under the sheet being turned instead of a blank page. Does it feel like a real book, and does anything flash?
• Please keep testing as before: your part, Without me, tempo, loop, page turns and Performance mode.
• Tell us what was hard, broken or missing (screenshot feedback in TestFlight). Mention the piece and the bar if you can.

## Feedback email and other fields

- **Feedback email:** choirbird-support@nikielhard.se
- **Marketing URL:** https://nikielhard.se/en/choirbird (Swedish: https://nikielhard.se/se/choirbird)
- **Privacy Policy URL:** https://nikielhard.se/en/choirbird/privacy. ChoirBird collects no data, has no tracking and makes no network calls; scores stay on the device or in a folder you choose (see `app/src-tauri/gen/apple/PrivacyInfo.xcprivacy`). Note recognition also runs on the device.
- **Sign-in required:** no.
- **Beta App Review notes:** ChoirBird needs no account. To try it, open the included Lacrymosa and press Play. To try note recognition, open any PDF score without MusicXML and tap "Read the notes"; it runs on the device and takes about a minute per page.
- **Licence agreement:** the GNU AGPL-3.0 with the app store permission (`LICENSE`, `LICENSE-EXCEPTIONS.md`).
