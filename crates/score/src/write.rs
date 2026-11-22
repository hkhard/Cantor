//! MusicXML writer for the normalised model, and score joining.
//!
//! Writes exactly what the model holds (divisions = PPQ), so
//! `parse(write(s)) == s`. Used to save OMR results that were assembled
//! page by page, and later for user corrections (WP-3.13).

use crate::model::{EventKind, Measure, Navigation, Part, Score, Step, PPQ};
use std::fmt::Write;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn step_name(s: Step) -> &'static str {
    match s {
        Step::C => "C",
        Step::D => "D",
        Step::E => "E",
        Step::F => "F",
        Step::G => "G",
        Step::A => "A",
        Step::B => "B",
    }
}

/// Serialise to an uncompressed `score-partwise` document.
pub fn write_musicxml(score: &Score) -> String {
    let mut x = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<score-partwise version=\"4.0\">\n  <part-list>\n",
    );
    for p in &score.parts {
        let _ = write!(
            x,
            "    <score-part id=\"{}\"><part-name>{}</part-name>",
            esc(&p.id),
            esc(&p.name)
        );
        if p.midi_program.is_some() || p.midi_channel.is_some() {
            let _ = write!(x, "<midi-instrument id=\"{}-I1\">", esc(&p.id));
            if let Some(c) = p.midi_channel {
                let _ = write!(x, "<midi-channel>{}</midi-channel>", c + 1);
            }
            if let Some(pr) = p.midi_program {
                let _ = write!(x, "<midi-program>{}</midi-program>", pr + 1);
            }
            x.push_str("</midi-instrument>");
        }
        x.push_str("</score-part>\n");
    }
    x.push_str("  </part-list>\n");

    for p in &score.parts {
        let _ = writeln!(x, "  <part id=\"{}\">", esc(&p.id));
        let mut last_time: Option<(u32, u32)> = None;
        for (i, m) in p.measures.iter().enumerate() {
            write_measure(&mut x, m, i == 0, &mut last_time);
        }
        x.push_str("  </part>\n");
    }
    x.push_str("</score-partwise>\n");
    x
}

fn write_measure(x: &mut String, m: &Measure, first: bool, last_time: &mut Option<(u32, u32)>) {
    let _ = write!(x, "    <measure number=\"{}\"", esc(&m.number));
    if m.implicit {
        x.push_str(" implicit=\"yes\"");
    }
    x.push('>');
    let nav = &m.nav;
    if nav.repeat_start || !nav.ending.is_empty() {
        x.push_str("<barline location=\"left\">");
        if !nav.ending.is_empty() {
            let nums: Vec<String> = nav.ending.iter().map(u32::to_string).collect();
            let _ = write!(x, "<ending number=\"{}\" type=\"start\"/>", nums.join(", "));
        }
        if nav.repeat_start {
            x.push_str("<repeat direction=\"forward\"/>");
        }
        x.push_str("</barline>");
    }
    if first || m.time_signature != *last_time {
        let _ = write!(x, "<attributes><divisions>{PPQ}</divisions>");
        if let Some((b, bt)) = m.time_signature {
            let _ = write!(
                x,
                "<time><beats>{b}</beats><beat-type>{bt}</beat-type></time>"
            );
        }
        x.push_str("</attributes>");
        *last_time = m.time_signature;
    }
    write_sound_nav(x, nav);
    for &(off, bpm) in &m.tempos {
        // Tempo at an offset: move there with forward/backup is overkill for
        // OMR output; tempos are written at measure start + offset 0 only when
        // offset is 0, else via a <sound> after a <forward>.
        if off == 0 {
            let _ = write!(x, "<sound tempo=\"{bpm}\"/>");
        } else {
            let _ = write!(
                x,
                "<forward><duration>{off}</duration></forward><sound tempo=\"{bpm}\"/><backup><duration>{off}</duration></backup>"
            );
        }
    }

    // Events in stored order. Restore the cursor with backup/forward so each
    // event lands at its offset; chords share the previous onset.
    let mut cursor = 0u64;
    // (offset, duration, voice, staff, was a note) of the last non-chord event
    let mut prev: Option<(u64, u64, u32, u32, bool)> = None;
    for e in &m.events {
        let is_note = matches!(e.kind, EventKind::Note(_));
        let is_chord = is_note
            && prev.is_some_and(|(o, d, v, s, was_note)| {
                was_note
                    && o == e.offset
                    && d == e.duration
                    && v == e.voice
                    && s == e.staff
                    && cursor == o + d
            });
        if !is_chord {
            if e.offset < cursor {
                let _ = write!(
                    x,
                    "<backup><duration>{}</duration></backup>",
                    cursor - e.offset
                );
            } else if e.offset > cursor {
                let _ = write!(
                    x,
                    "<forward><duration>{}</duration></forward>",
                    e.offset - cursor
                );
            }
        }
        x.push_str("<note>");
        if is_chord {
            x.push_str("<chord/>");
        }
        match &e.kind {
            EventKind::Rest => x.push_str("<rest/>"),
            EventKind::Note(p) => {
                let _ = write!(x, "<pitch><step>{}</step>", step_name(p.step));
                if p.alter != 0 {
                    let _ = write!(x, "<alter>{}</alter>", p.alter);
                }
                let _ = write!(x, "<octave>{}</octave></pitch>", p.octave);
            }
        }
        let _ = write!(x, "<duration>{}</duration>", e.duration);
        if e.tie_stop {
            x.push_str("<tie type=\"stop\"/>");
        }
        if e.tie_start {
            x.push_str("<tie type=\"start\"/>");
        }
        let _ = write!(x, "<voice>{}</voice>", e.voice);
        // Note type and tuplet ratio derived from the duration: some readers
        // (MuseScore) reject notes whose duration has no matching <type>.
        if let Some((ty, dots, tuplet)) = note_type(e.duration) {
            let _ = write!(x, "<type>{ty}</type>");
            for _ in 0..dots {
                x.push_str("<dot/>");
            }
            if let Some((a, n)) = tuplet {
                let _ = write!(
                    x,
                    "<time-modification><actual-notes>{a}</actual-notes><normal-notes>{n}</normal-notes></time-modification>"
                );
            }
        }
        let _ = write!(x, "<staff>{}</staff>", e.staff);
        for l in &e.lyrics {
            let _ = write!(x, "<lyric number=\"{}\">", l.verse);
            if !l.text.is_empty() {
                let _ = write!(
                    x,
                    "<syllabic>{}</syllabic><text>{}</text>",
                    l.syllabic.as_str(),
                    esc(&l.text)
                );
            }
            if l.extend {
                x.push_str("<extend/>");
            }
            x.push_str("</lyric>");
        }
        x.push_str("</note>");
        if !is_chord {
            cursor = e.offset + e.duration;
            prev = Some((e.offset, e.duration, e.voice, e.staff, is_note));
        }
    }
    // Pad to the content length so content_duration survives the round trip.
    if cursor < m.content_duration {
        let _ = write!(
            x,
            "<forward><duration>{}</duration></forward>",
            m.content_duration - cursor
        );
    }
    // An empty measure without a time signature would read back as length 0;
    // write a whole-measure rest so every reader keeps the parts aligned.
    if m.events.is_empty() && m.time_signature.is_none() && m.duration > 0 {
        let _ = write!(
            x,
            "<note><rest measure=\"yes\"/><duration>{}</duration><voice>1</voice></note>",
            m.duration
        );
    }
    if nav.repeat_end.is_some() || !nav.ending.is_empty() {
        x.push_str("<barline location=\"right\">");
        if !nav.ending.is_empty() {
            let nums: Vec<String> = nav.ending.iter().map(u32::to_string).collect();
            let kind = if nav.repeat_end.is_some() {
                "stop"
            } else {
                "discontinue"
            };
            let _ = write!(
                x,
                "<ending number=\"{}\" type=\"{kind}\"/>",
                nums.join(", ")
            );
        }
        if let Some(t) = nav.repeat_end {
            let _ = write!(x, "<repeat direction=\"backward\" times=\"{t}\"/>");
        }
        x.push_str("</barline>");
    }
    x.push_str("</measure>\n");
}

/// (type name, dots, tuplet `(actual, normal)`)
type NoteType = (&'static str, u32, Option<(u32, u32)>);

/// Graphic note type for a duration in ticks: plain, dotted (1-2 dots), or a
/// triplet/quintuplet/sextuplet of a plain value. `None` if nothing fits.
fn note_type(ticks: u64) -> Option<NoteType> {
    const TYPES: [(&str, u64); 9] = [
        ("breve", 8 * PPQ),
        ("whole", 4 * PPQ),
        ("half", 2 * PPQ),
        ("quarter", PPQ),
        ("eighth", PPQ / 2),
        ("16th", PPQ / 4),
        ("32nd", PPQ / 8),
        ("64th", PPQ / 16),
        ("128th", PPQ / 32),
    ];
    for &(name, base) in &TYPES {
        if ticks == base {
            return Some((name, 0, None));
        }
        if ticks == base * 3 / 2 {
            return Some((name, 1, None));
        }
        if ticks == base * 7 / 4 {
            return Some((name, 2, None));
        }
    }
    for &(actual, normal) in &[(3u64, 2u64), (5, 4), (6, 4), (7, 4)] {
        for &(name, base) in &TYPES {
            if ticks * actual == base * normal {
                return Some((name, 0, Some((actual as u32, normal as u32))));
            }
        }
    }
    None
}

fn write_sound_nav(x: &mut String, n: &Navigation) {
    let mut attrs = String::new();
    let mut add = |k: &str, v: &Option<String>| {
        if let Some(v) = v {
            let _ = write!(attrs, " {k}=\"{}\"", esc(v));
        }
    };
    add("segno", &n.segno);
    add("coda", &n.coda);
    add("tocoda", &n.tocoda);
    add("dalsegno", &n.dalsegno);
    if n.dacapo {
        attrs.push_str(" dacapo=\"yes\"");
    }
    if n.fine {
        attrs.push_str(" fine=\"yes\"");
    }
    if !attrs.is_empty() {
        let _ = write!(x, "<sound{attrs}/>");
    }
}

impl Score {
    /// Append another score's measures (e.g. the next OMR page). Parts are
    /// matched by position; a part missing on either side gets empty measures
    /// shaped like the other parts' measures so all parts stay aligned.
    pub fn append(&mut self, other: Score) {
        let before = self.measure_count();
        for p in other.parts.iter().skip(self.parts.len()) {
            self.parts.push(Part {
                measures: Vec::new(),
                ..p.clone()
            });
        }
        let mut incoming: Vec<Vec<Measure>> = other.parts.into_iter().map(|p| p.measures).collect();
        incoming.resize_with(self.parts.len(), Vec::new);
        for (part, inc) in self.parts.iter_mut().zip(incoming) {
            part.measures.resize_with(before, Measure::default);
            part.measures.extend(inc);
        }
        self.pad_parts();
    }

    /// Give every part the same number of measures. Missing or placeholder
    /// measures become empty measures copying the longest measure at that index.
    fn pad_parts(&mut self) {
        let n = self.measure_count();
        let shapes: Vec<Measure> = (0..n)
            .map(|k| {
                self.parts
                    .iter()
                    .filter_map(|p| p.measures.get(k))
                    .max_by_key(|m| m.duration)
                    .map(|m| Measure {
                        number: m.number.clone(),
                        duration: m.duration,
                        implicit: m.implicit,
                        time_signature: m.time_signature,
                        ..Measure::default()
                    })
                    .unwrap_or_default()
            })
            .collect();
        for part in &mut self.parts {
            part.measures.resize_with(n, Measure::default);
            for (m, shape) in part.measures.iter_mut().zip(&shapes) {
                if *m == Measure::default() {
                    *m = shape.clone();
                }
            }
        }
    }

    /// Number of measures in the longest part.
    pub fn measure_count(&self) -> usize {
        self.parts
            .iter()
            .map(|p| p.measures.len())
            .max()
            .unwrap_or(0)
    }
}
