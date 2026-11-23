# ChoirBird on TestFlight

Text to paste into App Store Connect › ChoirBird › TestFlight. Swedish
first (the choir is Swedish), English below. Limits: Beta App Description
4,000 characters, "What to Test" 4,000.

## Public link

**Public link:** _not created yet._ Create it in App Store Connect ›
TestFlight › External Testing › (group) › Public Link, then paste it here:

`https://testflight.apple.com/join/XXXXXXXX`

**Before turning on the public link** (it gives the app to people outside
the team, so these must be true first):

1. **Source must be public (AGPL section 6):** everyone who gets a build
   must be able to get its exact source. Make `hkhard/Cantor` public (or push
   a clean public copy) and check that About › Source points at the tag of
   the build (build 0.2.0 → tag `v0.2.0`). See `docs/ROADMAP.md`, phase 0.
2. **Clear *The Anchor Song* fixture** before the repository goes public
   (Björk's song; only the arrangement is ours).
3. **External testing needs Beta App Review** by Apple for the first build:
   allow about a day.
4. **Builds with on-device note recognition (homr)** are a separate
   question: homr's authors have not granted an app store permission. Do not
   put such a build on the public link until that is settled (see
   `crates/homr/README.md`). Build 0.2.0 does not contain homr.

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

## What to Test (build 0.2.0)

Svenska:
• Importera några av dina egna noter (PDF, MusicXML eller forScore .4sc/.4ss) och öppna dem.
• Öva med din stämma: Min stämma, Utan mig, tempo och slinga. Följer markören musiken i PDF:en?
• Vänd blad genom att svepa och genom att trycka i kanterna. Prova "Tryck flyttar en takt" under •••.
• Prova läget Framför i en repertoarlista: hoppa mellan sidor och stycken.
• På iPhone: täcker något noterna? Fungerar övningspanelen (⌃) och liggande läge?
• Berätta vad som var svårt, gick fel eller saknas (skärmbild i TestFlight).

English:
• Import some of your own scores (PDF, MusicXML or forScore .4sc/.4ss) and open them.
• Practise your part: My part, Without me, tempo and loop. Does the cursor follow the music on the PDF?
• Turn pages by swiping and by tapping the edges. Try "Tap moves one bar" under •••.
• Try Performance mode in a setlist: jump between pages and pieces.
• On iPhone: does anything cover the music? Do the practice sheet (⌃) and landscape work?
• Tell us what was hard, broken or missing (screenshot feedback in TestFlight).

## Feedback email and other fields

- **Feedback email:** _(fill in)_
- **Marketing URL / Privacy Policy URL:** needed for external testing. The privacy statement is short: ChoirBird collects no data, has no tracking and makes no network calls; scores stay on the device or in a folder you choose (see `app/src-tauri/gen/apple/PrivacyInfo.xcprivacy`).
- **Sign-in required:** no.
- **Licence agreement:** the GNU AGPL-3.0 with the app store permission (`LICENSE`, `LICENSE-EXCEPTIONS.md`).
