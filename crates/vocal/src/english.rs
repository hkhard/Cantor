//! English grapheme-to-phoneme for sung lyrics (ARPAbet with stress digits).
//!
//! Source: the CMU Pronouncing Dictionary (BSD-style licence, see
//! `data/cmudict.LICENSE`), bundled gzipped. Lyrics from scores and OCR need
//! more than a plain lookup, so in order:
//! 1. normalise: lower-case, curly quotes to `'`, strip punctuation;
//! 2. lyric elisions: `ev'ry` -> shortest variant of `every` (sung as written);
//! 3. exact lookup;
//! 4. compounds written together (`snowcovered`, OCR-merged `lightskeep`):
//!    split into the fewest dictionary words.

use crate::soulx::Phonemizer;
use std::collections::HashMap;
use std::io::Read;

static BUNDLED: &[u8] = include_bytes!("../data/cmudict.dict.gz");

pub struct EnglishPhonemizer {
    /// word -> pronunciation variants (first is the main one)
    dict: HashMap<String, Vec<Vec<String>>>,
}

impl EnglishPhonemizer {
    /// Parse CMUdict text: `word PH PH ...`, variants as `word(2)`, `;;;` comments,
    /// trailing `# comment` ignored.
    pub fn from_cmudict(text: &str) -> Self {
        let mut dict: HashMap<String, Vec<Vec<String>>> = HashMap::new();
        for line in text.lines() {
            if line.starts_with(";;;") {
                continue;
            }
            let line = line.split('#').next().unwrap_or("");
            let mut it = line.split_whitespace();
            let Some(head) = it.next() else { continue };
            let word = head.split('(').next().unwrap_or(head).to_lowercase();
            let phones: Vec<String> = it.map(str::to_string).collect();
            if !phones.is_empty() {
                dict.entry(word).or_default().push(phones);
            }
        }
        Self { dict }
    }

    /// The full CMU dictionary compiled into the binary (~135k words).
    pub fn bundled() -> Self {
        let mut text = String::new();
        flate2::read::GzDecoder::new(BUNDLED)
            .read_to_string(&mut text)
            .expect("bundled cmudict is valid gzip UTF-8");
        Self::from_cmudict(&text)
    }

    fn lookup(&self, w: &str) -> Option<Vec<String>> {
        self.dict.get(w).and_then(|v| v.first()).cloned()
    }

    /// Lyric spellings that drop a vowel (`ev'ry`, `heav'n`, `o'er`): use the
    /// full word's variant with the fewest vowels.
    fn elision(&self, w: &str) -> Option<Vec<String>> {
        let (a, b) = w.split_once('\'')?;
        if b.is_empty()
            || b == "s"
            || b == "t"
            || b == "ll"
            || b == "re"
            || b == "ve"
            || b == "d"
            || b == "m"
        {
            return None; // ordinary contraction, looked up as is
        }
        let vowels = |p: &Vec<String>| p.iter().filter(|x| x.ends_with(['0', '1', '2'])).count();
        ["e", "a", "o", "i", "u", "ve"]
            .iter()
            .filter_map(|fill| self.dict.get(&format!("{a}{fill}{b}")))
            .flat_map(|vars| vars.iter())
            .min_by_key(|p| vowels(p))
            .cloned()
    }

    /// Split into the fewest dictionary words (each at least 2 letters).
    fn compound(&self, w: &str) -> Option<Vec<String>> {
        let chars: Vec<char> = w.chars().collect();
        let n = chars.len();
        if n < 4 {
            return None;
        }
        // best[i] = (parts, phones) for the prefix of length i
        let mut best: Vec<Option<(usize, Vec<String>)>> = vec![None; n + 1];
        best[0] = Some((0, Vec::new()));
        for i in 0..n {
            let Some((parts, phones)) = best[i].clone() else {
                continue;
            };
            for j in (i + 2)..=n {
                let piece: String = chars[i..j].iter().collect();
                if let Some(p) = self.lookup(&piece) {
                    let cand = (parts + 1, [phones.clone(), p].concat());
                    if best[j].as_ref().is_none_or(|b| cand.0 < b.0) {
                        best[j] = Some(cand);
                    }
                }
            }
        }
        best[n]
            .clone()
            .filter(|(parts, _)| *parts >= 2)
            .map(|(_, p)| p)
    }
}

/// Lower-case, unify apostrophes (OCR also reads `’` as `”`), drop other punctuation.
pub fn normalise_word(w: &str) -> String {
    w.chars()
        .map(|c| match c {
            '\u{2019}' | '\u{2018}' | '`' | '\u{00B4}' | '\u{201D}' | '\u{201C}' => '\'',
            c => c,
        })
        .filter(|c| c.is_alphanumeric() || *c == '\'')
        .collect::<String>()
        .trim_matches('\'')
        .to_lowercase()
}

impl Phonemizer for EnglishPhonemizer {
    fn phonemes(&self, word: &str) -> Option<Vec<String>> {
        let w = normalise_word(word);
        if w.is_empty() {
            return None;
        }
        self.elision(&w)
            .or_else(|| self.lookup(&w))
            .or_else(|| self.lookup(&w.replace('\'', "")))
            .or_else(|| self.compound(&w.replace('\'', "")))
    }
}
