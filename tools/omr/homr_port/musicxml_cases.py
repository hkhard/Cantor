"""Reference MusicXML from homr 0.7.1 for synthetic token cases, for the
Rust port's stage 7 (crates/homr/tests/musicxml_cases.rs).

The cases are made up here (no score is copied): they use what the
Lacrymosa page does not, e.g. triplets, unmarked triplets, repeats, voltas,
grace notes, multi-measure rests, two voices on a staff, clef changes.

Run with homr's Python:
  python musicxml_cases.py fixtures/omr/homr_musicxml_cases.json
"""

import json
import sys
import xml.etree.ElementTree as ET

from homr.music_xml_generator import XmlGeneratorArguments, generate_xml
from homr.staff_parsing import _clean_up_voice
from homr.transformer.vocabulary import EncodedSymbol


def sym(text):
    parts = text.split()
    parts += ["."] * (6 - len(parts))
    return EncodedSymbol(*parts)


def bar(*notes):
    return [*notes, "barline"]


CASES = {
    "marked_triplets": [
        [
            "clef_G2 _ _ _ _ upper", "keySignature_2", "timeSignature/4",
            *bar("note_12 C5 _ _ _ upper", "note_12 D5 # _ _ upper", "note_12 E5 _ _ _ upper",
                 "note_4 F5 _ staccato _ upper", "rest_4 _ _ _ _ upper"),
            *bar("note_6 G4 _ _ _ upper", "note_12 A4 _ _ _ upper", "note_2 B4 b _ _ upper"),
        ]
    ],
    "unmarked_triplets_against_quarters": [
        [
            "clef_G2 _ _ _ _ upper", "chord", "clef_F4 _ _ _ _ lower", "timeSignature/4",
            *[t for _ in range(3) for t in bar(*[
                x for _ in range(3) for x in (
                    "note_8 C5 _ _ _ upper", "chord", "note_4 C3 _ _ _ lower",
                    "note_8 D5 _ _ _ upper", "note_8 E5 _ _ _ upper")
            ])],
        ]
    ],
    "repeats_voltas_dynamics": [
        [
            "clef_F4 _ _ _ _ upper", "keySignature_-3", "timeSignature/4",
            "repeatStart",
            "dynamic_f _ _ _ _ upper",
            "note_2 C3 _ accent_staccato _ upper", "note_2 D3 _ fermata _ upper",
            "repeatEnd",
            "voltaStart",
            "note_1 E3 N tenuto _ upper",
            "voltaStop",
            "bolddoublebarline",
            "dynamic_empty _ _ dynamic_piu _ upper",
            "rest_3m _ _ _ _ upper",
            "barline",
            "note_2 F3 _ trill _ upper", "note_2 G3 _ tremolo _ upper",
            "doublebarline",
            "note_1 A3 _ _ _ upper",
            "repeatEndStart",
            "note_1 B3 _ _ _ upper",
            "barline",
        ]
    ],
    "grace_ties_slurs_two_voices": [
        [
            "clef_G2 _ _ _ _ upper", "timeSignature/8",
            *bar("note_8G D5 _ _ _ upper", "note_4. C5 _ _ slurStart upper", "chord",
                 "note_4. E4 _ _ slurStart upper2",
                 "note_8 C5 _ _ slurStop upper", "chord", "note_8 F4 _ _ slurStop upper2",
                 "note_8 B4 _ _ slurStart_slurStop upper", "note_8 A4 _ _ slurStop upper"),
            *bar("note_4 G4 _ _ _ upper", "chord", "rest_8 _ _ _ _ upper2",
                 "note_8 C4 _ _ _ upper2", "note_4. B4 _ _ _ upper"),
        ]
    ],
    "piano_clef_change_and_mixed_chord": [
        [
            "clef_G2 _ _ _ _ upper", "chord", "clef_F4 _ _ _ _ lower",
            "keySignature_1", "timeSignature/4",
            *bar("note_4 G4 _ _ _ upper", "chord", "note_4 B4 _ _ _ upper", "chord",
                 "note_2 G2 _ _ _ lower",
                 "note_4 A4 _ _ _ upper",
                 "note_4 B4 _ _ _ upper", "chord", "note_4 D3 _ _ _ lower",
                 "note_4 C5 _ _ _ upper", "chord", "clef_G2 _ _ _ _ lower",
                 "chord", "rest_4 _ _ _ _ lower"),
            *bar("note_1 D5 _ _ _ upper", "chord", "note_1 D4 _ _ _ lower"),
            "newline",
            "clef_G2 _ _ _ _ upper", "chord", "clef_F4 _ _ _ _ lower", "keySignature_1",
            *bar("rest_1 _ _ _ _ upper", "chord", "rest_0 _ _ _ _ lower"),
            *bar("note_2 G4 _ _ _ upper", "chord", "note_2 G3 _ _ _ lower",
                 "note_2 F4 # _ _ upper", "chord", "note_2 A3 _ _ _ lower", "barline"),
        ]
    ],
    "two_parts_no_time_signature": [
        [
            "clef_C3 _ _ _ _ upper",
            *bar("note_4 C4 _ _ _ upper", "note_4 D4 _ _ _ upper", "note_2 E4 _ _ _ upper"),
            "newline",
            *bar("note_16 F4 _ _ _ upper", "note_16 G4 _ _ _ upper", "note_8 A4 _ _ _ upper",
                 "note_4.. B4 _ _ _ upper", "note_32 C5 _ _ _ upper", "note_4 D5 _ _ _ upper"),
        ],
        [
            "clef_F4 _ _ _ _ upper",
            "measureRest _ _ _ _ upper", "barline",
            "newline",
            *bar("note_1 C3 _ _ _ upper"),
        ],
    ],
}

out = {}
for name, voices in CASES.items():
    parsed = [[sym(t) for t in v] for v in voices]
    cleaned = [_clean_up_voice(v) for v in parsed]
    xml = generate_xml(XmlGeneratorArguments(False, None, None), cleaned, "")
    text = "<?xml version='1.0' encoding='utf-8'?>\n" + ET.tostring(xml, encoding="unicode")
    out[name] = {"voices": voices, "musicxml": text}
json.dump(out, open(sys.argv[1], "w"), indent=1, ensure_ascii=False)
print(len(out), "cases written to", sys.argv[1])
