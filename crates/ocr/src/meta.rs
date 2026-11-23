//! Title-page reader: turn recognised lines of a score's first page into
//! metadata. Rules follow how choir music is printed in Swedish and English:
//! the title is the biggest text near the top, credits are short lines with
//! a role word ("Musik:", "Arr.", "Text:", "Words by"), and a bare credit at
//! the right is the composer.

use crate::Line;
use serde::Serialize;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct TitlePage {
    pub title: String,
    pub subtitle: String,
    pub composer: String,
    pub arranger: String,
    pub lyricist: String,
    /// "SATB", "TTBB" or "SSAA" when the page says which choir it is for.
    pub choir: Option<&'static str>,
}

#[derive(Clone, Copy, PartialEq)]
enum Role {
    Composer,
    Arranger,
    Lyricist,
    Both, // "Text och musik", "Words and music"
}

/// Role words, longest first so "Text och musik" wins over "Text".
const ROLES: &[(&str, Role)] = &[
    ("text och musik", Role::Both),
    ("ord och musik", Role::Both),
    ("words and music by", Role::Both),
    ("words and music", Role::Both),
    ("music by", Role::Composer),
    ("composed by", Role::Composer),
    ("tonsättning", Role::Composer),
    ("komp.", Role::Composer),
    ("musik", Role::Composer),
    ("music", Role::Composer),
    ("melodi", Role::Composer),
    ("arrangemang", Role::Arranger),
    ("arranged by", Role::Arranger),
    ("arrangement", Role::Arranger),
    ("bearbetning", Role::Arranger),
    ("bearb.", Role::Arranger),
    ("sats", Role::Arranger),
    ("arr.", Role::Arranger),
    ("arr", Role::Arranger),
    ("words by", Role::Lyricist),
    ("lyrics by", Role::Lyricist),
    ("lyrics", Role::Lyricist),
    ("words", Role::Lyricist),
    ("text", Role::Lyricist),
    ("ord", Role::Lyricist),
    ("dikt", Role::Lyricist),
];

/// Words that are never a name on their own ("by" from "Arranged by").
const NOT_NAMES: &[&str] = &["by", "av", "af", "och", "and", "von", "de"];

/// `Some((role, name))` for a credit line; the name is empty when the role
/// word stands alone ("Arranged by") and the name is on the next line.
fn credit(text: &str) -> Option<(Role, String)> {
    let lower = text.to_lowercase();
    for (word, role) in ROLES {
        if let Some(rest) = lower.strip_prefix(word) {
            // A whole word: followed by ':', '.', ' ' or the end.
            let next = rest.chars().next();
            if word.ends_with('.') || matches!(next, None | Some(':') | Some(' ') | Some('.')) {
                let cut = text.len() - rest.len();
                let mut name = text[cut..].trim_start_matches([':', '.', ' ']).trim();
                // "by" / Swedish "af"/"av" after the role word.
                for w in ["by ", "af ", "av "] {
                    if name.to_lowercase().starts_with(w) {
                        name = name[w.len()..].trim();
                    }
                }
                let name = clean_name(name);
                if NOT_NAMES.contains(&name.to_lowercase().as_str()) {
                    return Some((*role, String::new()));
                }
                return Some((*role, name));
            }
        }
    }
    None
}

/// "Jacob Arcadelt (1507–1568)" -> "Jacob Arcadelt"; trailing years and
/// punctuation dropped; ALL CAPS becomes Title Case.
fn clean_name(s: &str) -> String {
    let s = match s.find('(') {
        Some(i) => &s[..i],
        None => s,
    };
    let s = s
        .trim_end_matches(|c: char| c.is_ascii_digit() || " -–—,.:".contains(c))
        .trim();
    title_case_if_caps(s)
}

fn title_case_if_caps(s: &str) -> String {
    // "and GLORIA SHAYNE" is all caps apart from the joining word.
    let letters: Vec<char> = s
        .split(' ')
        .filter(|w| !["and", "och"].contains(w))
        .flat_map(|w| w.chars())
        .filter(|c| c.is_alphabetic())
        .collect();
    if letters.len() < 4 || letters.iter().any(|c| c.is_lowercase()) {
        return s.to_string();
    }
    s.split(' ')
        .map(|w| {
            let lw = w.to_lowercase();
            if ["and", "och"].contains(&lw.as_str()) {
                return lw;
            }
            let mut c = lw.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn choir_of(text: &str) -> Option<&'static str> {
    let l = text.to_lowercase();
    let words: Vec<&str> = l.split(|c: char| !c.is_alphanumeric()).collect();
    let has = |ws: &[&str]| words.iter().any(|w| ws.contains(w));
    if has(&["ttbb", "tbb", "ttb", "tttbb", "ttbbb"])
        || ["manskör", "male choir", "men's choir", "male voices"]
            .iter()
            .any(|k| l.contains(k))
    {
        return Some("TTBB");
    }
    if has(&["ssaa", "ssa"])
        || ["damkör", "female choir", "women's choir", "treble choir"]
            .iter()
            .any(|k| l.contains(k))
    {
        return Some("SSAA");
    }
    if has(&["satb", "ssatb", "satbb", "ssaattbb"])
        || ["blandad kör", "mixed choir", "mixed chorus", "mixed voices"]
            .iter()
            .any(|k| l.contains(k))
    {
        return Some("SATB");
    }
    None
}

const TEMPO_WORDS: &[&str] = &[
    "andante",
    "allegro",
    "adagio",
    "moderato",
    "moderately",
    "lento",
    "largo",
    "presto",
    "rubato",
    "lugnt",
    "livligt",
    "flowing",
    "slowly",
    "slow",
    "brightly",
    "gently",
    "con ",
    "tempo",
    "maestoso",
    "grave",
    "vivace",
    "lively",
];
const VOICES: &[&str] = &[
    "sopran", "soprano", "alt", "alto", "tenor", "bas", "bass", "baryton", "baritone", "piano",
    "s.", "a.", "t.", "b.",
];

/// The first line that belongs to the music (tempo mark, voice name,
/// lyrics syllables, verse number): the title block is above it.
fn is_music(l: &Line) -> bool {
    let t = l.text.trim();
    let lower = t.to_lowercase();
    let first = lower.split_whitespace().next().unwrap_or("");
    TEMPO_WORDS.iter().any(|w| lower.starts_with(w))
        || (t.contains('=') && t.chars().any(|c| c.is_ascii_digit()))
        || VOICES
            .iter()
            .any(|v| first == *v || first.trim_end_matches(|c: char| c.is_ascii_digit()) == *v)
        || t.contains(" - ")
        || t.matches('-').count() >= 2
        || first.len() <= 3
            && first.ends_with('.')
            && first[..first.len() - 1].chars().all(|c| c.is_ascii_digit())
            && !first.is_empty()
            && first != "."
}

/// Lines that are never a title or a name: copyright, page numbers,
/// durations, garbage from note heads.
fn is_noise(l: &Line) -> bool {
    let t = l.text.trim();
    let lower = t.to_lowercase();
    let letters = t.chars().filter(|c| c.is_alphabetic()).count();
    let chars = t.chars().filter(|c| !c.is_whitespace()).count().max(1);
    letters < 4
        || letters * 10 < chars * 4 // mostly symbols or digits
        || lower.starts_with("copyright")
        || t.starts_with('©')
        || lower.contains("all rights")
        || lower.starts_with("duration")
        || lower.starts_with("page ")
        || lower.starts_with("this arrangement")
        || is_music(l)
}

fn same_column(a: &Line, b: &Line) -> bool {
    (a.x - b.x).abs() < 0.05 || ((a.x + a.w) - (b.x + b.w)).abs() < 0.05
}

/// The line just below `i` in the same column, if close.
fn below(lines: &[Line], pool: &[usize], i: usize) -> Option<usize> {
    let a = &lines[i];
    pool.iter()
        .copied()
        .filter(|&j| {
            j != i
                && lines[j].y > a.y + a.h * 0.4
                && lines[j].y - (a.y + a.h) < a.h * 1.5 + 0.01
                && same_column(a, &lines[j])
        })
        .min_by(|&x, &y| lines[x].y.total_cmp(&lines[y].y))
}

/// Read title, subtitle and credits from the recognised lines of page 1.
pub fn guess_metadata(lines: &[Line]) -> TitlePage {
    let mut order: Vec<usize> = (0..lines.len()).collect();
    order.sort_by(|&a, &b| lines[a].y.total_cmp(&lines[b].y));
    // The title block: everything above the first music line (but at least
    // the top 5%, page headers can look like anything) and above 35%.
    let cutoff = order
        .iter()
        .map(|&i| &lines[i])
        .find(|l| l.y > 0.05 && is_music(l))
        .map(|l| l.y)
        .unwrap_or(0.35)
        .min(0.35);
    // A line belongs to the block when it ends above the music too: note
    // heads read as text ("l slepai.") start above the first voice name
    // but reach down into the staff.
    let top: Vec<usize> = order
        .iter()
        .copied()
        .filter(|&i| lines[i].y < cutoff && lines[i].y + lines[i].h < cutoff + 0.01)
        .collect();
    let mut m = TitlePage {
        choir: top.iter().find_map(|&i| choir_of(&lines[i].text)),
        ..TitlePage::default()
    };
    let mut used = vec![false; lines.len()];
    let set = |m: &mut TitlePage, role: Role, name: String| {
        let slot = match role {
            Role::Composer => &mut m.composer,
            Role::Arranger => &mut m.arranger,
            Role::Lyricist => &mut m.lyricist,
            Role::Both => {
                if m.lyricist.is_empty() {
                    m.lyricist = name.clone();
                }
                &mut m.composer
            }
        };
        if slot.is_empty() {
            *slot = name;
        }
    };
    for &i in &top {
        if used[i] {
            continue;
        }
        let Some((role, mut name)) = credit(&lines[i].text) else {
            continue;
        };
        used[i] = true;
        let mut last = i;
        if name.is_empty() {
            // "Arranged by" / "ROBERT STERLING" on the next line.
            match below(lines, &top, i) {
                Some(j) if !used[j] => {
                    used[j] = true;
                    name = clean_name(&lines[j].text);
                    last = j;
                }
                _ => continue,
            }
        }
        // Names continued on the next line: "SMITH," / "CINDY MORGAN and WES KING",
        // "NOEL REGNEY" / "and GLORIA SHAYNE".
        if let Some(j) = below(lines, &top, last) {
            let next = lines[j].text.trim();
            if !used[j]
                && (lines[last].text.trim_end().ends_with(',')
                    || next.to_lowercase().starts_with("and ")
                    || next.to_lowercase().starts_with("och "))
            {
                used[j] = true;
                let sep = if next.to_lowercase().starts_with("and ")
                    || next.to_lowercase().starts_with("och ")
                {
                    " "
                } else {
                    ", "
                };
                name = format!("{name}{sep}{}", clean_name(next));
            }
        }
        set(&mut m, role, name);
    }
    // Title: the tallest line in the block that is not noise, a credit or a
    // bare choir label ("TTBB").
    let title = top
        .iter()
        .copied()
        .filter(|&i| !used[i] && !is_noise(&lines[i]))
        .filter(|&i| {
            choir_of(&lines[i].text).is_none() || lines[i].text.split_whitespace().count() > 2
        })
        .max_by(|&a, &b| lines[a].h.total_cmp(&lines[b].h));
    let Some(ti) = title else { return m };
    used[ti] = true;
    // Parts of the title on the same row at a similar size ("Fredmans" +
    // "Epistel Nº 72."), joined left to right.
    let t = lines[ti].clone();
    let mut row: Vec<usize> = top
        .iter()
        .copied()
        .filter(|&i| {
            !used[i]
                && !is_noise(&lines[i])
                && (lines[i].y + lines[i].h / 2.0 - (t.y + t.h / 2.0)).abs() < t.h / 2.0
                && lines[i].h > t.h * 0.6
        })
        // Close by (a word gap), not a name printed far right on the same row.
        .filter(|&i| {
            let l = &lines[i];
            let gap = if l.x > t.x {
                l.x - (t.x + t.w)
            } else {
                t.x - (l.x + l.w)
            };
            gap < t.h * 3.0 && !l.text.contains('(')
        })
        .collect();
    row.push(ti);
    row.sort_by(|&a, &b| lines[a].x.total_cmp(&lines[b].x));
    for &i in &row {
        used[i] = true;
    }
    m.title = title_case_if_caps(
        &row.iter()
            .map(|&i| lines[i].text.trim().trim_end_matches([':', ' ']))
            .collect::<Vec<_>>()
            .join(" "),
    );
    let (tx0, tx1) = (
        row.iter().map(|&i| lines[i].x).fold(1.0, f32::min),
        row.iter()
            .map(|&i| lines[i].x + lines[i].w)
            .fold(0.0, f32::max),
    );
    let cx = (tx0 + tx1) / 2.0;
    let looks_like_name = |s: &str| {
        let ws: Vec<&str> = s.split_whitespace().collect();
        (2..=4).contains(&ws.len())
            && ws
                .iter()
                .all(|w| w.chars().next().is_some_and(|c| c.is_uppercase()))
    };
    // Subtitle: the closest centred line below the title, unless it is a
    // bare name (then it is the composer, "Olle Adolphsson").
    let sub = top
        .iter()
        .copied()
        .filter(|&i| !used[i] && !is_noise(&lines[i]))
        .filter(|&i| {
            let l = &lines[i];
            l.y > t.y && l.y - (t.y + t.h) < 0.06 && ((l.x + l.w / 2.0) - cx).abs() < 0.08
        })
        .min_by(|&a, &b| lines[a].y.total_cmp(&lines[b].y));
    if let Some(si) = sub {
        used[si] = true;
        let s = lines[si].text.trim();
        if looks_like_name(s) && choir_of(s).is_none() && m.composer.is_empty() {
            m.composer = clean_name(s);
        } else {
            m.subtitle = s.to_string();
        }
    }
    // Bare names beside or under the title: on the right the composer
    // (then the lyricist under it), on the left the lyricist. This is how
    // choir music is usually printed ("Bjørnson" left, "Reissiger" right).
    let bare = |side_right: bool, used: &Vec<bool>| -> Vec<usize> {
        let mut v: Vec<usize> = top
            .iter()
            .copied()
            .filter(|&i| !used[i] && !is_noise(&lines[i]) && choir_of(&lines[i].text).is_none())
            .filter(|&i| {
                let l = &lines[i];
                l.y + l.h > t.y
                    && if side_right {
                        l.x > 0.5 && l.x + l.w > 0.75
                    } else {
                        l.x + l.w < 0.5 && l.x < 0.2
                    }
            })
            .collect();
        v.sort_by(|&a, &b| lines[a].y.total_cmp(&lines[b].y));
        v
    };
    let right = bare(true, &used);
    let left = bare(false, &used);
    // The first right-hand line is the composer even when it is not a name
    // ("Svensk folkmelodi", "Trad."); a second one must look like a name.
    let mut right_names = right.iter().map(|&i| clean_name(&lines[i].text));
    if m.composer.is_empty() {
        if let Some(n) = right_names.next() {
            m.composer = n;
        }
    }
    let mut right_names = right_names.filter(|s| looks_like_name(s));
    if m.lyricist.is_empty() {
        let l = left
            .iter()
            .map(|&i| clean_name(&lines[i].text))
            .find(|s| looks_like_name(s))
            .or_else(|| right_names.next());
        if let Some(n) = l {
            m.lyricist = n;
        }
    }
    m
}
