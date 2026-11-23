//! homr `transformer/vocabulary.py`, the parts the MusicXML stage needs:
//! [`Sym`] (`EncodedSymbol`), durations (`SymbolDuration`,
//! `kern_to_symbol_duration`), `sort_token_chords` and
//! `remove_duplicated_symbols` (with the over-eager tuplet fix).
use super::fraction::Frac;

pub const NONOTE: &str = ".";
pub const EMPTY: &str = "_";

/// A decoded symbol split into the decoder's branches (homr `EncodedSymbol`).
#[derive(Clone, Debug)]
pub struct Sym {
    pub rhythm: String,
    pub pitch: String,
    pub lift: String,
    pub articulation: String,
    pub slur: String,
    pub position: String,
    /// Where the symbol is on the input image, if known (homr `image_coordinates`).
    pub image_xy: Option<(f64, f64)>,
}

impl PartialEq for Sym {
    fn eq(&self, o: &Self) -> bool {
        self.rhythm == o.rhythm
            && self.pitch == o.pitch
            && self.lift == o.lift
            && self.articulation == o.articulation
            && self.slur == o.slur
            && self.position == o.position
    }
}

/// homr `SymbolDuration`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Duration {
    pub dots: u32,
    pub actual_notes: i64,
    pub normal_notes: i64,
    pub fraction: Frac,
    pub kern: i64,
}

impl Duration {
    fn new(base: Frac, dots: u32, actual: i64, normal: i64, kern: i64) -> Duration {
        let an = Frac::new(actual, normal);
        let (actual_notes, normal_notes) = (an.numer(), an.denom());
        let mut dur = base;
        let mut add = base / Frac::int(2);
        for _ in 0..dots {
            dur = dur + add;
            add = add / Frac::int(2);
        }
        if actual_notes != normal_notes {
            dur = dur * Frac::new(normal_notes, actual_notes);
        }
        Duration {
            dots,
            actual_notes,
            normal_notes,
            fraction: dur,
            kern,
        }
    }
    pub fn is_tuplet(&self) -> bool {
        self.actual_notes != self.normal_notes
    }
}

fn prior_power_of_two(n: i64) -> i64 {
    if n < 1 {
        return 1;
    }
    1 << (63 - n.leading_zeros())
}

/// homr `kern_to_symbol_duration`.
pub fn kern_to_duration(kern: &str) -> Duration {
    if kern.ends_with('m') {
        return Duration::new(Frac::int(1), 0, 1, 1, 4);
    }
    let digits: String = kern.chars().take_while(|c| c.is_ascii_digit()).collect();
    let base: i64 = digits.parse().unwrap_or(4);
    let dots = kern.matches('.').count() as u32;
    if kern.contains('G') {
        return Duration::new(Frac::ZERO, dots, 1, 1, base);
    }
    if base == 0 {
        return Duration::new(Frac::int(1), dots, 1, 1, base);
    }
    if base & (base - 1) == 0 {
        Duration::new(Frac::new(1, base), dots, 1, 1, base)
    } else {
        let normal = prior_power_of_two(base);
        Duration::new(Frac::new(1, normal), dots, base, normal, normal)
    }
}

impl Sym {
    pub fn new(rhythm: &str) -> Sym {
        Sym {
            rhythm: rhythm.to_string(),
            pitch: NONOTE.into(),
            lift: NONOTE.into(),
            articulation: NONOTE.into(),
            slur: NONOTE.into(),
            position: NONOTE.into(),
            image_xy: None,
        }
    }
    pub fn with_position(rhythm: &str, position: &str) -> Sym {
        let mut s = Sym::new(rhythm);
        s.position = position.into();
        s
    }

    /// Parses homr's text form, e.g. `"note_4 C4 _ _ _ upper"`.
    pub fn parse(text: &str) -> Sym {
        let mut it = text.split_whitespace();
        let mut next = || it.next().unwrap_or(NONOTE).to_string();
        Sym {
            rhythm: next(),
            pitch: next(),
            lift: next(),
            articulation: next(),
            slur: next(),
            position: next(),
            image_xy: None,
        }
    }

    pub fn is_note_or_rest(&self) -> bool {
        self.rhythm.starts_with("note") || self.rhythm.starts_with("rest")
    }
    pub fn is_barline_or_repeat(&self) -> bool {
        self.rhythm.contains("barline") || self.rhythm.contains("repeat")
    }

    /// homr `get_duration`.
    pub fn duration(&self) -> Duration {
        if !self.is_note_or_rest() {
            return Duration::new(Frac::ZERO, 0, 1, 1, 1);
        }
        kern_to_duration(self.rhythm.split('_').nth(1).unwrap_or(""))
    }
    pub fn fraction(&self) -> Frac {
        self.duration().fraction
    }

    /// homr `remove_tuplet`.
    pub fn remove_tuplet(&self) -> Sym {
        let Some((kind, rest)) = self.rhythm.split_once('_') else {
            return self.clone();
        };
        if kind != "note" && kind != "rest" {
            return self.clone();
        }
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        let Ok(d) = digits.parse::<i64>() else {
            return self.clone();
        };
        let d = if d % 3 == 0 {
            d / 3 * 2
        } else if d % 5 == 0 {
            d / 5 * 4
        } else if d % 7 == 0 {
            d / 7 * 4
        } else {
            return self.clone();
        };
        let mut r = self.clone();
        r.rhythm = format!("{kind}_{d}{}", &rest[digits.len()..]);
        r
    }

    /// homr `to_upper_position`.
    pub fn to_upper_position(&self) -> Sym {
        let mut r = self.clone();
        if is_lower_position(&self.position) {
            r.position = self.position.replace("lower", "upper");
        }
        r
    }
}

impl std::fmt::Display for Sym {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} {} {} {} {}",
            self.rhythm, self.pitch, self.lift, self.articulation, self.slur, self.position
        )
    }
}

pub fn is_lower_position(position: &str) -> bool {
    position.starts_with("lower")
}

/// Groups symbols joined by `chord` (homr `_group_into_chords`).
pub fn group_into_chords(symbols: &[Sym]) -> Vec<Vec<Sym>> {
    let mut chords: Vec<Vec<Sym>> = Vec::new();
    let mut in_chord = false;
    for s in symbols {
        if s.rhythm == "chord" {
            in_chord = true;
        } else if in_chord && !chords.is_empty() {
            chords.last_mut().unwrap().push(s.clone());
            in_chord = false;
        } else {
            chords.push(vec![s.clone()]);
        }
    }
    chords
}

/// homr `sort_token_chords`: the chords, each sorted as Python sorts
/// `EncodedSymbol`s (descending by text, stable).
pub fn sort_token_chords(symbols: &[Sym]) -> Vec<Vec<Sym>> {
    sort_token_chords_indexed(symbols)
        .into_iter()
        .map(|c| c.into_iter().map(|i| symbols[i].clone()).collect())
        .collect()
}

/// [`sort_token_chords`] as indices into `symbols`.
pub fn sort_token_chords_indexed(symbols: &[Sym]) -> Vec<Vec<usize>> {
    let mut chords: Vec<Vec<usize>> = Vec::new();
    let mut in_chord = false;
    for (i, s) in symbols.iter().enumerate() {
        if s.rhythm == "chord" {
            in_chord = true;
        } else if in_chord && !chords.is_empty() {
            chords.last_mut().unwrap().push(i);
            in_chord = false;
        } else {
            chords.push(vec![i]);
        }
    }
    for c in chords.iter_mut() {
        let keys: Vec<String> = c.iter().map(|&i| symbols[i].to_string()).collect();
        let mut order: Vec<usize> = (0..c.len()).collect();
        order.sort_by(|&a, &b| keys[b].cmp(&keys[a]));
        *c = order.into_iter().map(|k| c[k]).collect();
    }
    chords
}

fn flatten_chords(chords: Vec<Vec<Sym>>) -> Vec<Sym> {
    let mut out = Vec::new();
    for chord in chords {
        for (i, s) in chord.into_iter().enumerate() {
            if i > 0 {
                out.push(Sym::new("chord"));
            }
            out.push(s);
        }
    }
    out
}

fn group_into_measures(chords: Vec<Vec<Sym>>) -> Vec<Vec<Vec<Sym>>> {
    let mut measures = Vec::new();
    let mut current = Vec::new();
    for chord in chords {
        let ends = chord.first().is_some_and(Sym::is_barline_or_repeat);
        current.push(chord);
        if ends {
            measures.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        measures.push(current);
    }
    measures
}

fn duration_of_measure(measure: &[Vec<Sym>]) -> Frac {
    let mut total = Frac::ZERO;
    for chord in measure {
        let mut d = Frac::ZERO;
        for s in chord.iter().filter(|s| s.is_note_or_rest()) {
            let f = s.fraction();
            if f > Frac::ZERO && (f < d || d == Frac::ZERO) {
                d = f;
            }
        }
        total = total + d;
    }
    total
}

/// homr `_fix_over_eager_tuplets`: measures shorter than the typical one lose their tuplets.
fn fix_over_eager_tuplets(chords: Vec<Vec<Sym>>) -> Vec<Vec<Sym>> {
    let measures = group_into_measures(chords);
    let mut durations: Vec<Frac> = measures.iter().map(|m| duration_of_measure(m)).collect();
    durations.sort();
    let typical = durations
        .get(durations.len() / 2)
        .copied()
        .unwrap_or(Frac::ZERO);
    let mut out = Vec::new();
    for m in measures {
        if duration_of_measure(&m) < typical {
            out.extend(
                m.into_iter()
                    .map(|c| c.iter().map(Sym::remove_tuplet).collect::<Vec<_>>()),
            );
        } else {
            out.extend(m);
        }
    }
    out
}

/// homr `_only_keep_lower_staff_if_there_is_a_clef`, decided per staff line.
fn only_keep_lower_staff_if_there_is_a_clef(chords: Vec<Vec<Sym>>) -> Vec<Vec<Sym>> {
    let mut has_lower_clef = false;
    let mut line_start = 0;
    let mut out = Vec::with_capacity(chords.len());
    for (i, chord) in chords.into_iter().enumerate() {
        if chord.iter().any(|s| s.rhythm == "newline") {
            has_lower_clef = false;
            line_start = i + 1;
        }
        let mut result = Vec::with_capacity(chord.len());
        for s in chord {
            if has_lower_clef {
                result.push(s);
            } else if (i as isize - line_start as isize) < 5
                && s.rhythm.starts_with("clef")
                && is_lower_position(&s.position)
            {
                has_lower_clef = true;
                result.push(s);
            } else {
                result.push(s.to_upper_position());
            }
        }
        out.push(result);
    }
    out
}

/// homr `_remove_duplicated_piches`. homr's replacement of a shorter duplicate
/// writes to a key which is never read, so the first of each pitch and
/// position stays: so here too.
fn remove_duplicated_pitches(chord: Vec<Sym>) -> Vec<Sym> {
    if chord.len() <= 1 || !chord[0].is_note_or_rest() {
        return chord;
    }
    let mut seen: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for s in chord {
        let key = format!("{} {}", s.pitch, s.position);
        if !seen.contains(&key) {
            seen.push(key);
            out.push(s);
        }
    }
    out
}

fn remove_redundant_clefs_keys_and_time_signatures(chords: Vec<Vec<Sym>>) -> Vec<Vec<Sym>> {
    let (mut clef_upper, mut clef_lower, mut key, mut time) =
        (String::new(), String::new(), String::new(), String::new());
    chords
        .into_iter()
        .map(|chord| {
            let mut result = Vec::new();
            for s in chord {
                let slot = if s.rhythm.starts_with("clef") {
                    if is_lower_position(&s.position) {
                        Some(&mut clef_lower)
                    } else {
                        Some(&mut clef_upper)
                    }
                } else if s.rhythm.starts_with("keySignature") {
                    Some(&mut key)
                } else if s.rhythm.starts_with("timeSignature") {
                    Some(&mut time)
                } else {
                    None
                };
                match slot {
                    Some(last) if *last == s.rhythm => {}
                    Some(last) => {
                        *last = s.rhythm.clone();
                        result.push(s);
                    }
                    None => result.push(s),
                }
            }
            result
        })
        .collect()
}

/// homr `remove_duplicated_symbols(symbols, cleanup_tuplets=True)`.
pub fn remove_duplicated_symbols(symbols: &[Sym]) -> Vec<Sym> {
    let chords = group_into_chords(symbols);
    let chords = fix_over_eager_tuplets(chords);
    let chords = only_keep_lower_staff_if_there_is_a_clef(chords);
    let chords = chords.into_iter().map(remove_duplicated_pitches).collect();
    flatten_chords(remove_redundant_clefs_keys_and_time_signatures(chords))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lib_06_kern_durations_as_homr() {
        let d = kern_to_duration("4.");
        assert_eq!((d.fraction, d.dots, d.kern), (Frac::new(3, 8), 1, 4));
        let t = kern_to_duration("12");
        assert_eq!((t.actual_notes, t.normal_notes, t.kern), (3, 2, 8));
        assert_eq!(t.fraction, Frac::new(1, 12));
        assert_eq!(kern_to_duration("8G").fraction, Frac::ZERO);
        assert_eq!(kern_to_duration("0").fraction, Frac::int(1));
        assert_eq!(kern_to_duration("3m").fraction, Frac::int(1));
        assert_eq!(
            Sym::parse("note_12. C4 _ _ _ upper").remove_tuplet().rhythm,
            "note_8."
        );
        assert_eq!(
            Sym::parse("note_8 C4 _ _ _ upper").remove_tuplet().rhythm,
            "note_8"
        );
    }

    #[test]
    fn lib_06_chords_sort_descending_by_text() {
        let s: Vec<Sym> = [
            "note_8 A4 _ _ _ upper",
            "chord",
            "note_8 C5 _ _ _ upper",
            "barline",
        ]
        .iter()
        .map(|t| Sym::parse(t))
        .collect();
        let c = sort_token_chords(&s);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0][0].pitch, "C5");
        assert_eq!(c[0][1].pitch, "A4");
    }

    #[test]
    fn lib_06_lower_staff_needs_a_lower_clef() {
        let s: Vec<Sym> = ["clef_G2 _ _ _ _ upper", "note_4 C4 _ _ _ lower"]
            .iter()
            .map(|t| Sym::parse(t))
            .collect();
        let r = remove_duplicated_symbols(&s);
        assert_eq!(r[1].position, "upper");
    }
}
