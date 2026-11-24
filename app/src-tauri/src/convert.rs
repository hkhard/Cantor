//! LIB-10: convert a PDF score to playable notes on the device, in the
//! background. A queue of jobs runs on one worker thread (never on the main
//! thread or the UI thread); progress and completion are sent to the UI as
//! events, and a finished job posts a local notification (iOS) so the user
//! hears about it while doing something else in the app.
//!
//! The recogniser is a trait so the queue is tested without the 129 MB
//! models: [`PageRecognizer`] is implemented by `cantor-homr` in the app
//! build and by a stand-in in tests. Each page's MusicXML is joined into one
//! score (`<stem>.musicxml` next to the PDF), so the existing library,
//! pairing and PDF cursor (measure map from the layout) take over.
// Only builds with the `homr` feature start the queue; the rest is tested.
#![cfg_attr(not(feature = "homr"), allow(dead_code))]

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};

/// Recognises one page image (PNG) to MusicXML.
pub trait PageRecognizer: Send {
    fn recognize(&mut self, png: &[u8], progress: &mut dyn FnMut(f32)) -> Result<String, String>;
}

/// Renders the pages of a PDF to PNG images for recognition.
pub trait PdfPages: Send + Sync {
    fn pages(&self, pdf: &[u8]) -> Result<Vec<Vec<u8>>, String>;
}

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JobEvent {
    /// `fraction` of the whole job (all pages), 0..1.
    Progress {
        id: String,
        title: String,
        page: usize,
        pages: usize,
        fraction: f32,
    },
    Done {
        id: String,
        title: String,
        measures: usize,
    },
    Failed {
        id: String,
        title: String,
        error: String,
    },
}

/// One queued conversion: the score id (library), its title and PDF.
#[derive(Debug, Clone)]
pub struct Job {
    pub id: String,
    pub title: String,
    pub pdf: PathBuf,
}

/// The queue. `submit` returns at once; work happens on the worker thread.
#[derive(Clone)]
pub struct Converter {
    tx: Sender<Job>,
    pub busy: Arc<Mutex<Vec<String>>>,
}

impl Converter {
    /// Start the worker. `make` builds the recogniser on the worker thread
    /// (model loading is slow and must not block the caller); `emit` is
    /// called from the worker for every event.
    pub fn start<R, P, M, E>(make: M, pdf: P, emit: E) -> Self
    where
        R: PageRecognizer + 'static,
        M: FnOnce() -> Result<R, String> + Send + 'static,
        P: PdfPages + 'static,
        E: Fn(JobEvent) + Send + 'static,
    {
        let (tx, rx) = channel::<Job>();
        let busy: Arc<Mutex<Vec<String>>> = Arc::default();
        let busy2 = busy.clone();
        std::thread::Builder::new()
            .name("choirbird-convert".into())
            .spawn(move || {
                let mut rec: Option<Result<R, String>> = None;
                let mut make = Some(make);
                for job in rx {
                    if rec.is_none() {
                        rec = Some(make.take().expect("once")());
                    }
                    let ev = match rec.as_mut().expect("set") {
                        Ok(r) => run(r, &pdf, &job, &emit),
                        Err(e) => JobEvent::Failed {
                            id: job.id.clone(),
                            title: job.title.clone(),
                            error: e.clone(),
                        },
                    };
                    crate::guard::lock(&busy2).retain(|x| x != &job.id);
                    emit(ev);
                }
            })
            .expect("spawn converter");
        Converter { tx, busy }
    }

    /// Queue a job (ignored if the same score is already queued).
    pub fn submit(&self, job: Job) {
        let mut b = crate::guard::lock(&self.busy);
        if b.contains(&job.id) {
            return;
        }
        b.push(job.id.clone());
        drop(b);
        let _ = self.tx.send(job);
    }
}

fn run<R: PageRecognizer, P: PdfPages>(
    rec: &mut R,
    pdf: &P,
    job: &Job,
    emit: &dyn Fn(JobEvent),
) -> JobEvent {
    let fail = |e: String| JobEvent::Failed {
        id: job.id.clone(),
        title: job.title.clone(),
        error: e,
    };
    let bytes = match std::fs::read(&job.pdf) {
        Ok(b) => b,
        Err(e) => return fail(e.to_string()),
    };
    let pages = match pdf.pages(&bytes) {
        Ok(p) if !p.is_empty() => p,
        Ok(_) => return fail("the PDF has no pages".into()),
        Err(e) => return fail(e),
    };
    let n = pages.len();
    let mut xmls = Vec::with_capacity(n);
    for (i, png) in pages.iter().enumerate() {
        let mut last = -1.0f32;
        let mut on = |f: f32| {
            // At most ~50 events per page.
            if f - last >= 0.02 || f >= 1.0 {
                last = f;
                emit(JobEvent::Progress {
                    id: job.id.clone(),
                    title: job.title.clone(),
                    page: i + 1,
                    pages: n,
                    fraction: (i as f32 + f.clamp(0.0, 1.0)) / n as f32,
                });
            }
        };
        on(0.0);
        match rec.recognize(png, &mut on) {
            Ok(x) => xmls.push(x),
            // A page without music (title page, lyrics) is skipped.
            Err(e) if e.contains("no staffs") => continue,
            Err(e) => return fail(format!("page {}: {e}", i + 1)),
        }
    }
    if xmls.is_empty() {
        return fail("no music found on any page".into());
    }
    let joined = match join_pages(&xmls) {
        Ok(x) => x,
        Err(e) => return fail(e),
    };
    let measures = cantor_score::parse_musicxml(joined.as_bytes())
        .map(|s| s.parts.first().map_or(0, |p| p.measures.len()))
        .unwrap_or(0);
    let out = musicxml_path(&job.pdf);
    let tmp = out.with_extension("musicxml.part");
    if let Err(e) = std::fs::write(&tmp, &joined).and_then(|_| std::fs::rename(&tmp, &out)) {
        return fail(e.to_string());
    }
    JobEvent::Done {
        id: job.id.clone(),
        title: job.title.clone(),
        measures,
    }
}

/// `<stem>.musicxml` next to the PDF: the library pairs them into one score.
pub fn musicxml_path(pdf: &Path) -> PathBuf {
    pdf.with_extension("musicxml")
}

/// Join per-page MusicXML into one score. Pages can show different parts (a
/// soprano solo, then S T B, then a baritone joins), so the parts of each
/// page are matched to the score's lines by clef, top to bottom: a page part
/// goes to the first unused line with its clef after the previous match, or
/// becomes a new line there. Lines a page doesn't show get a measure rest per
/// measure, so every part keeps the same bars. Measure numbers continue.
pub fn join_pages(pages: &[String]) -> Result<String, String> {
    if pages.len() == 1 {
        return Ok(pages[0].clone());
    }
    // Each line: its clef sign and its measures so far.
    let mut lines: Vec<(char, Vec<String>)> = Vec::new();
    let mut total = 0usize;
    for page in pages {
        let parts = split_parts(page)?;
        let bars = parts.iter().map(|p| measures(p).len()).max().unwrap_or(0);
        // Match the page's parts to the lines in order, as many as possible by
        // clef (longest common subsequence); unmatched parts become new lines
        // just before the next matched one.
        let signs: Vec<char> = parts.iter().map(|p| clef_sign(p)).collect();
        let pairs = align(&lines.iter().map(|l| l.0).collect::<Vec<_>>(), &signs);
        let mut placed: Vec<(usize, Vec<String>)> = Vec::new();
        let mut inserted = 0usize;
        for (k, part) in parts.iter().enumerate() {
            let l = match pairs.iter().find(|p| p.1 == k) {
                Some(&(l, _)) => l + inserted,
                None => {
                    // Before the next matched line, or at the end.
                    let at = pairs
                        .iter()
                        .find(|p| p.1 > k)
                        .map_or(lines.len(), |&(l, _)| l + inserted);
                    lines.insert(at, (signs[k], rest_bars(total, signs[k])));
                    for p in placed.iter_mut() {
                        if p.0 >= at {
                            p.0 += 1;
                        }
                    }
                    inserted += 1;
                    at
                }
            };
            let mut ms = measures(part);
            ms.resize_with(bars, || rest_measure(None));
            placed.push((l, ms));
        }
        for (l, line) in lines.iter_mut().enumerate() {
            match placed.iter().position(|p| p.0 == l) {
                Some(k) => line.1.extend(std::mem::take(&mut placed[k].1)),
                None => line.1.extend((0..bars).map(|_| rest_measure(Some(line.0)))),
            }
        }
        total += bars;
    }
    // Number the measures and write the score with the first page's header.
    let first = &pages[0];
    let head_end = first.find("<part-list>").ok_or("no <part-list>")?;
    let list: String = (0..lines.len())
        .map(|k| {
            format!(
                r#"<score-part id="P{0}"><part-name>Voice</part-name></score-part>"#,
                k + 1
            )
        })
        .collect();
    let body: String = lines
        .iter()
        .enumerate()
        .map(|(k, (_, ms))| {
            format!(
                r#"<part id="P{}">{}</part>"#,
                k + 1,
                renumber(&ms.concat(), 0)
            )
        })
        .collect();
    Ok(format!(
        "{}<part-list>{list}</part-list>{body}</score-partwise>",
        &first[..head_end]
    ))
}

/// The longest order-preserving matching of `lines` to `parts` by equal
/// clef: pairs (line index, part index). Ties keep the earliest lines.
fn align(lines: &[char], parts: &[char]) -> Vec<(usize, usize)> {
    let (n, m) = (lines.len(), parts.len());
    let mut t = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            t[i][j] = if lines[i] == parts[j] {
                t[i + 1][j + 1] + 1
            } else {
                t[i + 1][j].max(t[i][j + 1])
            };
        }
    }
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < n && j < m {
        if lines[i] == parts[j] && t[i][j] == t[i + 1][j + 1] + 1 {
            out.push((i, j));
            i += 1;
            j += 1;
        } else if t[i][j + 1] >= t[i + 1][j] {
            j += 1;
        } else {
            i += 1;
        }
    }
    out
}

/// The clef sign (G, F, C) a part starts with, G if none is given.
fn clef_sign(part: &str) -> char {
    part.find("<sign>")
        .and_then(|i| part[i + 6..].chars().next())
        .unwrap_or('G')
}

/// The `<measure>` elements of a part, in order.
fn measures(part: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = part;
    while let Some(i) = rest.find("<measure") {
        let Some(e) = rest[i..].find("</measure>") else {
            break;
        };
        let e = e + i + "</measure>".len();
        out.push(rest[i..e].to_string());
        rest = &rest[e..];
    }
    out
}

/// A whole-bar rest; with `clef`, it also sets the clef (a line that starts
/// later still shows its clef in its first bar).
fn rest_measure(clef: Option<char>) -> String {
    let attr = match clef {
        Some('F') => "<attributes><divisions>1</divisions><clef><sign>F</sign><line>4</line></clef></attributes>",
        Some('C') => "<attributes><divisions>1</divisions><clef><sign>C</sign><line>3</line></clef></attributes>",
        Some(_) => "<attributes><divisions>1</divisions><clef><sign>G</sign><line>2</line></clef></attributes>",
        None => "",
    };
    format!(
        r#"<measure number="0">{attr}<note><rest measure="yes"/><duration>4</duration><voice>1</voice></note></measure>"#
    )
}

/// `n` whole-bar rests for a line that joins on a later page.
fn rest_bars(n: usize, sign: char) -> Vec<String> {
    (0..n)
        .map(|i| rest_measure(if i == 0 { Some(sign) } else { None }))
        .collect()
}

fn split_parts(xml: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(s) = rest.find("<part ") {
        let e = rest[s..].find("</part>").ok_or("unclosed <part>")? + s + "</part>".len();
        out.push(rest[s..e].to_string());
        rest = &rest[e..];
    }
    if out.is_empty() {
        return Err("no parts in the recognised page".into());
    }
    Ok(out)
}

fn renumber(measures: &str, offset: usize) -> String {
    let mut out = String::with_capacity(measures.len());
    let mut rest = measures;
    let mut n = offset;
    while let Some(i) = rest.find("<measure ") {
        out.push_str(&rest[..i]);
        let tag_end = rest[i..].find('>').map_or(rest.len(), |e| e + i + 1);
        n += 1;
        let tag = &rest[i..tag_end];
        let fixed = match tag.find("number=\"") {
            Some(p) => {
                let q = tag[p + 8..].find('"').map_or(tag.len(), |q| q + p + 8);
                format!("{}number=\"{n}\"{}", &tag[..p], &tag[q + 1..])
            }
            None => tag.to_string(),
        };
        out.push_str(&fixed);
        rest = &rest[tag_end..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod lib_10_app {
    use super::*;
    use std::sync::mpsc::Receiver;
    use std::time::Duration;

    fn page(notes: &[&str]) -> String {
        let ms: String = notes
            .iter()
            .enumerate()
            .map(|(i, s)| {
                format!(r#"<measure number="{}"><note><pitch><step>{s}</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type></note></measure>"#, i + 1)
            })
            .collect();
        format!(
            r#"<?xml version="1.0"?><score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Voice</part-name></score-part></part-list><part id="P1">{ms}</part></score-partwise>"#
        )
    }

    /// One part of a test page: clef sign, clef line, notes (step, octave;
    /// step "-" is a whole-bar rest).
    type TestPart<'a> = (&'a str, u8, &'a [(&'a str, u8)]);

    /// A page of parts.
    fn parts_page(parts: &[TestPart]) -> String {
        let list: String = (0..parts.len())
            .map(|k| {
                format!(
                    r#"<score-part id="P{0}"><part-name>Voice</part-name></score-part>"#,
                    k + 1
                )
            })
            .collect();
        let body: String = parts
            .iter()
            .enumerate()
            .map(|(k, (sign, line, notes))| {
                let ms: String = notes
                    .iter()
                    .enumerate()
                    .map(|(i, (step, oct))| {
                        let attr = if i == 0 {
                            format!("<attributes><divisions>1</divisions><clef><sign>{sign}</sign><line>{line}</line></clef></attributes>")
                        } else {
                            String::new()
                        };
                        if *step == "-" {
                            format!(r#"<measure number="{}">{attr}<note><rest measure="yes"/><duration>4</duration></note></measure>"#, i + 1)
                        } else {
                            format!(r#"<measure number="{}">{attr}<note><pitch><step>{step}</step><octave>{oct}</octave></pitch><duration>4</duration><type>whole</type></note></measure>"#, i + 1)
                        }
                    })
                    .collect();
                format!(r#"<part id="P{}">{ms}</part>"#, k + 1)
            })
            .collect();
        format!(
            r#"<?xml version="1.0"?><score-partwise version="4.0"><part-list>{list}</part-list>{body}</score-partwise>"#
        )
    }

    /// The notes of each part of a joined score, as "C4" or "-" for a rest.
    fn lines(xml: &str) -> Vec<Vec<String>> {
        split_parts(xml)
            .unwrap()
            .iter()
            .map(|p| {
                let mut out = Vec::new();
                let mut rest = p.as_str();
                while let Some(i) = rest.find("<note>") {
                    let e = rest[i..].find("</note>").unwrap() + i;
                    let n = &rest[i..e];
                    out.push(match (n.find("<step>"), n.find("<octave>")) {
                        (Some(a), Some(b)) => format!("{}{}", &n[a + 6..a + 7], &n[b + 8..b + 9]),
                        _ => "-".to_string(),
                    });
                    rest = &rest[e..];
                }
                out
            })
            .collect()
    }

    #[test]
    fn lib_10_pages_with_different_parts_join_by_line() {
        // Abide with me: page 1 has S, T, B; page 2 adds a baritone solo
        // (bass clef) between S and T. Each line must stay one part, and the
        // baritone gets rests on page 1.
        let p1 = parts_page(&[
            ("G", 2, &[("B", 4), ("G", 4)]),
            ("G", 2, &[("-", 0), ("D", 4)]),
            ("F", 4, &[("-", 0), ("G", 2)]),
        ]);
        let p2 = parts_page(&[
            ("G", 2, &[("C", 5)]),
            ("F", 4, &[("E", 3)]),
            ("G", 2, &[("E", 4)]),
            ("F", 4, &[("C", 3)]),
        ]);
        let joined = join_pages(&[p1, p2]).unwrap();
        assert_eq!(
            lines(&joined),
            vec![
                vec!["B4", "G4", "C5"],
                vec!["-", "-", "E3"],
                vec!["-", "D4", "E4"],
                vec!["-", "G2", "C3"],
            ]
        );
        cantor_score::parse_score_file(joined.as_bytes()).expect("parses");
    }

    struct Fake {
        pages: Vec<String>,
        calls: Arc<Mutex<usize>>,
        thread: Arc<Mutex<Option<String>>>,
    }
    impl PageRecognizer for Fake {
        fn recognize(
            &mut self,
            png: &[u8],
            progress: &mut dyn FnMut(f32),
        ) -> Result<String, String> {
            *self.thread.lock().unwrap() = std::thread::current().name().map(str::to_string);
            *self.calls.lock().unwrap() += 1;
            progress(0.5);
            std::thread::sleep(Duration::from_millis(20));
            progress(1.0);
            let i = png[0] as usize;
            if png == b"\x09blank" {
                return Err("no staffs found on the page".into());
            }
            Ok(self.pages[i].clone())
        }
    }
    struct Pages(Vec<Vec<u8>>);
    impl PdfPages for Pages {
        fn pages(&self, _pdf: &[u8]) -> Result<Vec<Vec<u8>>, String> {
            Ok(self.0.clone())
        }
    }

    fn setup(
        pdf_pages: Vec<Vec<u8>>,
    ) -> (
        Converter,
        Receiver<JobEvent>,
        PathBuf,
        Arc<Mutex<Option<String>>>,
    ) {
        let d = std::env::temp_dir().join(format!(
            "cb-conv-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("hymn.pdf"), b"%PDF-1.4").unwrap();
        let (etx, erx) = channel();
        let thread = Arc::new(Mutex::new(None));
        let t2 = thread.clone();
        let c = Converter::start(
            move || {
                Ok(Fake {
                    pages: vec![page(&["C", "D"]), page(&["E", "F", "G"])],
                    calls: Arc::default(),
                    thread: t2,
                })
            },
            Pages(pdf_pages),
            move |e| {
                let _ = etx.send(e);
            },
        );
        (c, erx, d, thread)
    }

    fn wait_done(rx: &Receiver<JobEvent>) -> (Vec<JobEvent>, JobEvent) {
        let mut seen = Vec::new();
        loop {
            let e = rx.recv_timeout(Duration::from_secs(10)).expect("event");
            match e {
                JobEvent::Progress { .. } => seen.push(e),
                _ => return (seen, e),
            }
        }
    }

    #[test]
    fn lib_10_converts_in_the_background_joins_pages_and_reports_done() {
        let (c, rx, d, thread) = setup(vec![vec![0], vec![1]]);
        let t0 = std::time::Instant::now();
        c.submit(Job {
            id: "1:hymn".into(),
            title: "Hymn".into(),
            pdf: d.join("hymn.pdf"),
        });
        // submit returns at once: the work is on another thread.
        assert!(t0.elapsed() < Duration::from_millis(15));
        let (progress, done) = wait_done(&rx);
        assert_eq!(
            done,
            JobEvent::Done {
                id: "1:hymn".into(),
                title: "Hymn".into(),
                measures: 5
            }
        );
        assert_eq!(thread.lock().unwrap().as_deref(), Some("choirbird-convert"));
        // Progress covers both pages and rises to 1.
        let fr: Vec<f32> = progress
            .iter()
            .map(|e| {
                if let JobEvent::Progress { fraction, .. } = e {
                    *fraction
                } else {
                    0.0
                }
            })
            .collect();
        assert!(fr.windows(2).all(|w| w[1] >= w[0]), "{fr:?}");
        assert!((fr.last().unwrap() - 1.0).abs() < 1e-6);
        // One score next to the PDF, measures numbered 1..5, parsable.
        let xml = std::fs::read_to_string(d.join("hymn.musicxml")).unwrap();
        let s = cantor_score::parse_musicxml(xml.as_bytes()).unwrap();
        assert_eq!(s.parts.len(), 1);
        assert_eq!(s.parts[0].measures.len(), 5);
        assert!(xml.contains(r#"<measure number="5">"#));
        assert!(!d.join("hymn.musicxml.part").exists());
    }

    #[test]
    fn lib_10_pages_without_music_are_skipped_and_errors_reported() {
        let (c, rx, d, _) = setup(vec![b"\x09blank".to_vec(), vec![0]]);
        c.submit(Job {
            id: "1:hymn".into(),
            title: "Hymn".into(),
            pdf: d.join("hymn.pdf"),
        });
        let (_, done) = wait_done(&rx);
        assert!(
            matches!(done, JobEvent::Done { measures: 2, .. }),
            "{done:?}"
        );
        // A missing PDF fails with a message, and the worker keeps going.
        c.submit(Job {
            id: "1:gone".into(),
            title: "Gone".into(),
            pdf: d.join("gone.pdf"),
        });
        let (_, f) = wait_done(&rx);
        assert!(matches!(f, JobEvent::Failed { .. }), "{f:?}");
        c.submit(Job {
            id: "1:hymn".into(),
            title: "Hymn".into(),
            pdf: d.join("hymn.pdf"),
        });
        assert!(matches!(wait_done(&rx).1, JobEvent::Done { .. }));
    }

    #[test]
    fn lib_10_a_model_that_fails_to_load_fails_the_job_not_the_app() {
        let (etx, erx) = channel();
        let c = Converter::start(
            || Err::<Fake, _>("models missing".to_string()),
            Pages(vec![vec![0]]),
            move |e| {
                let _ = etx.send(e);
            },
        );
        c.submit(Job {
            id: "1:x".into(),
            title: "X".into(),
            pdf: PathBuf::from("/nonexistent.pdf"),
        });
        let e = erx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(
            e,
            JobEvent::Failed {
                id: "1:x".into(),
                title: "X".into(),
                error: "models missing".into()
            }
        );
    }
}
