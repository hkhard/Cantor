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

/// Join per-page MusicXML into one score: the first page's part list, then
/// each page's measures appended to the matching part (by order), with
/// measure numbers continued.
pub fn join_pages(pages: &[String]) -> Result<String, String> {
    if pages.len() == 1 {
        return Ok(pages[0].clone());
    }
    let first = &pages[0];
    let mut parts: Vec<String> = split_parts(first)?;
    let mut numbers: Vec<usize> = parts
        .iter()
        .map(|p| p.matches("<measure ").count())
        .collect();
    for page in &pages[1..] {
        for (k, body) in split_parts(page)?.into_iter().enumerate() {
            if k >= parts.len() {
                break;
            }
            let renumbered = renumber(&inner(&body), numbers[k]);
            numbers[k] += renumbered.matches("<measure ").count();
            let close = parts[k].rfind("</part>").ok_or("no </part>")?;
            parts[k].insert_str(close, &renumbered);
        }
    }
    let head_end = first.find("<part ").ok_or("no <part>")?;
    let tail = first
        .rfind("</score-partwise>")
        .ok_or("no </score-partwise>")?;
    Ok(format!(
        "{}{}{}",
        &first[..head_end],
        parts.concat(),
        &first[tail..]
    ))
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

fn inner(part: &str) -> String {
    let s = part.find('>').map_or(0, |i| i + 1);
    let e = part.rfind("</part>").unwrap_or(part.len());
    part[s..e].to_string()
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
