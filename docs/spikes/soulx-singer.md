# Spike: SoulX-Singer as Cantor's score-accurate singing engine

Checked 2026-11-22 against github.com/Soul-AILab/SoulX-Singer at commit
`81aeb3a` (2026-05-29) and the Hugging Face model card.

## Verdict

> **Update 2026-11-22 (`singing-engines.md`):** SoulX-Singer is the default
> engine, chosen by ear over DiffSinger. 8 steps without CFG renders at RTF 1.1
> on an M2 GPU. Swedish and on-phone size remain open.

**Use it as the default "sing my part" engine for English, behind the
`VocalProvider` trait, run on a user GPU server. It cannot sing Swedish today.**
Swedish needs either a phoneme mapping onto its English set (approximate) or
another engine; this blocks the Swedish choir use case and must be tested
before the beta.

## What fits

| Need (PRD) | SoulX-Singer | Fit |
| --- | --- | --- |
| Follow the notated melody and rhythm (VOX-04) | "score" control mode takes per-note MIDI pitch + duration in seconds | Yes, exact |
| Melismas | `note_type` 3 = continuation of the previous syllable on a new note | Yes, maps 1:1 to MusicXML `syllabic`/extend |
| Rests / breaths | `<SP>` tokens with pitch 0, `note_type` 1 | Yes |
| Voice choice (VOX-09) | Zero-shot timbre from a prompt clip + its metadata | Yes, needs a reference clip **with** transcription |
| Licence (NFR-09) | Apache-2.0 for code and weights | Yes |
| Long pieces | Inference loops over segments (`time` [start_ms, end_ms]) and pastes them at their offsets | Yes; Cantor's phrases become segments (preprocess splits at 60 s) |
| Tempo / transpose (VOX-08) | Durations are seconds; `pitch_shift` / `auto_shift` | Yes; re-render only changed phrases |

## What does not fit

| Issue | Detail | Consequence |
| --- | --- | --- |
| **No Swedish** | Phone set has 2,820 symbols: 1,529 Cantonese, 1,201 Mandarin, 70 English (ARPAbet), 20 special. G2P handles only Chinese characters and `[A-Za-z']` words; any other word (e.g. "född", "så") is replaced by `<SP>` silence | Swedish lyrics are silently dropped. Need a Swedish to ARPAbet approximation (å/ä/ö/y/u have no ARPAbet equivalents) or another engine |
| CUDA only in practice | CLI defaults to `cuda`; fp16 only on CUDA; WebUI falls back to CPU. No MPS path | Runs on a user NVIDIA GPU server / HF Endpoint (as PRD planned). Mac CPU probably too slow; to measure |
| Model size | `model.pt` 2.82 GB (+ SVC 2.79 GB) | Not on-device; server only |
| Heavy Python stack | `nemo_toolkit`, `funasr`, `sageattention`, torch, Python 3.10 | Ship as a Docker image / HF Space template (PRD risk table already lists this) |
| Prompt needs metadata | The voice prompt must have phoneme, note_pitch, note_type, duration **and** f0 (their preprocess pipeline produces it: vocal separation, ASR, note transcription) | For "pick a voice" ship 3 to 5 prepared prompts; user clips go through their preprocess on the server |
| No hosted inference | The HF Space is a demo, not an API | Users run their own endpoint (PRD section 5 already assumes this) |

## Input contract Cantor must produce (target segment)

One JSON list, one object per segment (a Cantor phrase):

```json
{
  "index": "p0003",
  "language": "English",
  "time": [12340, 18900],
  "duration": "0.25 0.50 0.50 0.75 0.40",
  "text": "<SP> Glo ri a <SP>",
  "phoneme": "<SP> en_G-L-AO1 en_R-IY0 en_AH0 <SP>",
  "note_pitch": "0 67 69 67 0",
  "note_type": "1 2 2 3 1"
}
```

- Parallel space-separated lists, one entry per **note** (not per syllable).
- `note_type`: 1 = rest/`<SP>`, 2 = note that starts a syllable, 3 = melisma
  continuation (same syllable, new pitch). Text/phoneme for a type-3 note repeat
  the syllable's.
- `phoneme` for English is `en_` + ARPAbet joined with `-` per word (their G2P
  is per word; Cantor sends per syllable, so the word must be phonemised once
  and its phones distributed over its syllables).
- `f0` is optional for the target in score mode.
- `time` is the segment position in ms in the output; durations must sum to it.

## Plan changes

- `crates/lyrics` produces exactly this structure (notes with type 1/2/3 and
  durations from the Cantor timemap), so the SoulX adapter is a thin serialiser.
  Done in this round (VOX-01, VOX-03 serialiser).
- New risk: Swedish unsupported. Spike S2 now has a concrete question: can a
  Swedish->ARPAbet mapping (sv letters/digraphs to nearest en phones) give
  acceptable diction, or must Swedish use another engine (NNSVS with a Swedish
  voice, or a fine-tune)?
- Phrase length: SoulX preprocess caps segments at 60 s; Cantor phrases (split
  at rests >= 1 beat) are far shorter.
