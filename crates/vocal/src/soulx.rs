//! SoulX-Singer adapter: score-mode target metadata.
//!
//! Contract (docs/spikes/soulx-singer.md): a JSON list of segments, each with
//! space-separated parallel lists, one entry per note:
//! `duration` (s), `text`, `phoneme` (`en_` + ARPAbet joined by `-`),
//! `note_pitch` (MIDI, 0 for silence), `note_type` (1 silence, 2 new syllable,
//! 3 melisma continuation), plus `time` [start_ms, end_ms].
//!
//! SoulX-Singer has Mandarin, Cantonese and English phonemes only. Swedish is
//! rejected here rather than silently sung as silence.

use cantor_lyrics::{NoteKind, Phrase};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    Swedish,
}

#[derive(Debug, thiserror::Error)]
pub enum SoulxError {
    #[error(
        "SoulX-Singer cannot sing {0:?}: it only has Mandarin, Cantonese and English phonemes"
    )]
    UnsupportedLanguage(Language),
    #[error("no pronunciation for word {0:?}")]
    UnknownWord(String),
}

/// Grapheme-to-phoneme for whole words, ARPAbet with stress digits.
pub trait Phonemizer {
    fn phonemes(&self, word: &str) -> Option<Vec<String>>;
}

/// Dictionary lookup (CMUdict-style entries). The production phonemizer will
/// wrap CMUdict plus a fallback G2P model; this is enough for tests and tools.
pub struct DictPhonemizer {
    map: HashMap<String, Vec<String>>,
}

impl DictPhonemizer {
    pub fn from_pairs(pairs: &[(&str, &str)]) -> Self {
        Self {
            map: pairs
                .iter()
                .map(|(w, p)| {
                    (
                        normalise(w),
                        p.split_whitespace().map(str::to_string).collect(),
                    )
                })
                .collect(),
        }
    }

    /// Parse CMUdict text (`WORD  PH PH PH`, `;;;` comments, `WORD(2)` variants ignored).
    pub fn from_cmudict(text: &str) -> Self {
        let mut map = HashMap::new();
        for line in text.lines() {
            if line.starts_with(";;;") {
                continue;
            }
            let mut it = line.split_whitespace();
            let Some(word) = it.next() else { continue };
            if word.ends_with(')') {
                continue;
            }
            map.entry(normalise(word))
                .or_insert_with(|| it.map(str::to_string).collect());
        }
        Self { map }
    }
}

impl Phonemizer for DictPhonemizer {
    fn phonemes(&self, word: &str) -> Option<Vec<String>> {
        self.map.get(&normalise(word)).cloned()
    }
}

fn normalise(w: &str) -> String {
    w.chars()
        .filter(|c| c.is_alphanumeric() || *c == '\'')
        .collect::<String>()
        .to_lowercase()
}

fn is_vowel(ph: &str) -> bool {
    ph.ends_with(['0', '1', '2'])
}

/// Split a word's phonemes over `n` syllables at vowel nuclei. Intervocalic
/// consonants: one goes to the next syllable, of two or more the first stays
/// with the previous syllable. Leading consonants join the first syllable,
/// trailing ones the last.
pub fn split_syllables(phones: &[String], n: usize) -> Vec<Vec<String>> {
    if n <= 1 {
        return vec![phones.to_vec()];
    }
    let vowels: Vec<usize> = phones
        .iter()
        .enumerate()
        .filter(|(_, p)| is_vowel(p))
        .map(|(i, _)| i)
        .collect();
    if vowels.len() < 2 {
        // Not enough nuclei: whole word on the first syllable, the rest repeat the vowel.
        let mut out = vec![phones.to_vec()];
        let v = vowels
            .first()
            .map(|&i| phones[i].clone())
            .unwrap_or_default();
        out.extend((1..n).map(|_| vec![v.clone()]));
        return out;
    }
    // Boundaries between consecutive nuclei, at most n-1 of them (extra vowels
    // stay in the last syllable; missing ones repeat the last vowel).
    let mut cuts = Vec::new();
    for w in vowels.windows(2).take(n - 1) {
        let (a, b) = (w[0], w[1]);
        let consonants = b - a - 1;
        cuts.push(if consonants <= 1 { a + 1 } else { a + 2 });
    }
    let mut out = Vec::new();
    let mut start = 0;
    for c in cuts {
        out.push(phones[start..c].to_vec());
        start = c;
    }
    out.push(phones[start..].to_vec());
    while out.len() < n {
        let last_vowel = out
            .last()
            .and_then(|s| s.iter().rev().find(|p| is_vowel(p)).cloned())
            .unwrap_or_default();
        out.push(vec![last_vowel]);
    }
    out
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Segment {
    pub index: String,
    pub language: String,
    pub time: [u64; 2],
    pub duration: String,
    pub text: String,
    pub phoneme: String,
    pub note_pitch: String,
    pub note_type: String,
}

/// Phonemes used for sung notes without lyrics ("la", as SoulX's own MIDI import does).
const NO_LYRIC_TEXT: &str = "la";
const NO_LYRIC_PHONES: &str = "en_L-AA1";

/// Build SoulX-Singer target segments, one per phrase.
pub fn to_target_metadata(
    phrases: &[Phrase],
    language: Language,
    g2p: &dyn Phonemizer,
) -> Result<Vec<Segment>, SoulxError> {
    if language != Language::English {
        return Err(SoulxError::UnsupportedLanguage(language));
    }

    // Group syllables into words across the whole part (a word may span a
    // rest and therefore two phrases), phonemise once, split over syllables.
    let mut syllable_phones: HashMap<(usize, usize), String> = HashMap::new();
    let mut pending: Vec<((usize, usize), String)> = Vec::new();
    let flush = |pending: &mut Vec<((usize, usize), String)>,
                 out: &mut HashMap<(usize, usize), String>|
     -> Result<(), SoulxError> {
        if pending.is_empty() {
            return Ok(());
        }
        let word: String = pending.iter().map(|(_, t)| t.as_str()).collect();
        let phones = g2p
            .phonemes(&word)
            .ok_or_else(|| SoulxError::UnknownWord(normalise(&word)))?;
        for ((key, _), syl) in pending.iter().zip(split_syllables(&phones, pending.len())) {
            let joined: Vec<String> = syl.iter().map(|p| p.to_string()).collect();
            out.insert(*key, format!("en_{}", joined.join("-")));
        }
        pending.clear();
        Ok(())
    };
    for (pi, ph) in phrases.iter().enumerate() {
        for (ni, n) in ph.notes.iter().enumerate() {
            if n.kind == NoteKind::Syllable {
                pending.push(((pi, ni), n.text.clone()));
                if !n.syllabic.continues_word() {
                    flush(&mut pending, &mut syllable_phones)?;
                }
            }
        }
    }
    flush(&mut pending, &mut syllable_phones)?;

    let mut segs = Vec::new();
    for (pi, ph) in phrases.iter().enumerate() {
        let (mut dur, mut text, mut phon, mut pitch, mut typ) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut last_syllable_phones = NO_LYRIC_PHONES.to_string();
        let mut cursor = ph.start_sec;
        for (ni, n) in ph.notes.iter().enumerate() {
            let gap = n.start_sec - cursor;
            if gap > 0.005 {
                dur.push(format!("{gap:.2}"));
                text.push("<SP>".to_string());
                phon.push("<SP>".to_string());
                pitch.push("0".to_string());
                typ.push("1".to_string());
            }
            let (t, p, kind) = match n.kind {
                NoteKind::Syllable => {
                    let p = syllable_phones[&(pi, ni)].clone();
                    last_syllable_phones = p.clone();
                    (n.text.clone(), p, "2")
                }
                NoteKind::Melisma => (n.text.clone(), last_syllable_phones.clone(), "3"),
                NoteKind::NoLyric => (NO_LYRIC_TEXT.to_string(), NO_LYRIC_PHONES.to_string(), "2"),
            };
            dur.push(format!("{:.2}", n.duration_sec));
            text.push(t);
            phon.push(p);
            pitch.push(n.pitch.to_string());
            typ.push(kind.to_string());
            cursor = n.start_sec + n.duration_sec;
        }
        segs.push(Segment {
            index: format!("p{pi:04}"),
            language: "English".into(),
            time: [
                (ph.start_sec * 1000.0).round() as u64,
                (ph.end_sec * 1000.0).round() as u64,
            ],
            duration: dur.join(" "),
            text: text.join(" "),
            phoneme: phon.join(" "),
            note_pitch: pitch.join(" "),
            note_type: typ.join(" "),
        });
    }
    Ok(segs)
}

/// JSON document for SoulX-Singer's `--target_metadata_path`.
pub fn to_json(segments: &[Segment]) -> String {
    serde_json::to_string_pretty(segments).expect("segments serialise")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_string).collect()
    }

    #[test]
    fn syllable_split_rules() {
        assert_eq!(
            split_syllables(&v("G L AO1 R IY0 AH0"), 3),
            vec![v("G L AO1"), v("R IY0"), v("AH0")]
        );
        // Two intervocalic consonants: first stays with the previous syllable.
        assert_eq!(
            split_syllables(&v("HH AE1 P IY0"), 2),
            vec![v("HH AE1"), v("P IY0")]
        );
        assert_eq!(
            split_syllables(&v("W IH1 N T ER0"), 2),
            vec![v("W IH1 N"), v("T ER0")]
        );
        // More syllables than vowels: last vowel repeated.
        assert_eq!(
            split_syllables(&v("F AY1 ER0"), 3),
            vec![v("F AY1"), v("ER0"), v("ER0")]
        );
        assert_eq!(split_syllables(&v("AH0"), 1), vec![v("AH0")]);
    }

    #[test]
    fn cmudict_parsing() {
        let d =
            DictPhonemizer::from_cmudict(";;; comment\nAMEN  AA2 M EH1 N\nAMEN(2)  EY2 M EH1 N\n");
        assert_eq!(d.phonemes("Amen,"), Some(v("AA2 M EH1 N")));
    }
}
