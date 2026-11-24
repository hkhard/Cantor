//! LIB-10: the on-device recogniser, backed by cantor-homr (AGPL-only port
//! of homr). Only in builds with the `homr` feature.
use crate::convert::PageRecognizer;
use std::path::Path;

pub struct HomrRecognizer {
    models: cantor_homr::Models,
}

impl HomrRecognizer {
    /// Load the three models (on the worker thread: takes seconds, and the
    /// first CoreML compile after an install up to a minute).
    pub fn load(dir: &Path, cache: &Path) -> Result<Self, String> {
        let _ = std::fs::create_dir_all(cache);
        cantor_homr::Models::load(
            dir,
            cfg!(any(target_os = "ios", target_os = "macos")),
            cache,
        )
        .map(|models| HomrRecognizer { models })
        .map_err(|e| e.to_string())
    }
}

impl PageRecognizer for HomrRecognizer {
    fn recognize(&mut self, png: &[u8], progress: &mut dyn FnMut(f32)) -> Result<String, String> {
        let mut p = |_stage: &str, f: f32| progress(f);
        cantor_homr::recognize_page(&mut self.models, png, &mut p).map_err(|e| e.to_string())
    }
}

/// LIB-10 on the Mac with the real pieces: CoreGraphics renders the public
/// Lacrymosa PDF, homr reads every page, and the queue writes a MusicXML
/// our parser reads. Slow (about 10 s a page) and needs the models:
/// `cargo test --features homr-host --release --lib lib_10_real -- --ignored`.
#[cfg(test)]
mod lib_10_real {
    use super::*;
    use crate::apple_convert::CgPdfPages;
    use crate::convert::{Converter, Job, JobEvent};
    use std::time::{Duration, Instant};

    #[test]
    #[ignore]
    fn lib_10_real_lacrymosa_pdf_converts_on_the_mac() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let models = root.join("models/homr");
        let src = root.join(
            "fixtures/scores/mozart-lacrymosa/mozart-lacrymosa-from-requiem-satb-with-piano.pdf",
        );
        let dir = std::env::temp_dir().join(format!("cb-real-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let pdf = dir.join("lacrymosa.pdf");
        std::fs::copy(&src, &pdf).unwrap();
        let cache = dir.join("coreml");
        let (tx, rx) = std::sync::mpsc::channel();
        let c = Converter::start(
            move || HomrRecognizer::load(&models, &cache),
            CgPdfPages::default(),
            move |e| {
                let _ = tx.send(e);
            },
        );
        let t = Instant::now();
        c.submit(Job {
            id: "1:lacrymosa".into(),
            title: "Lacrymosa".into(),
            pdf: pdf.clone(),
        });
        loop {
            match rx.recv_timeout(Duration::from_secs(600)).expect("event") {
                JobEvent::Progress { page, pages, .. } => eprintln!(
                    "page {}/{pages} at {:.1} s",
                    page + 1,
                    t.elapsed().as_secs_f32()
                ),
                JobEvent::Done { measures, .. } => {
                    eprintln!(
                        "done: {measures} measures in {:.1} s",
                        t.elapsed().as_secs_f32()
                    );
                    assert!(measures > 10, "{measures}");
                    break;
                }
                JobEvent::Failed { error, .. } => panic!("failed: {error}"),
            }
        }
        let xml = std::fs::read(dir.join("lacrymosa.musicxml")).expect("written beside the PDF");
        let score = cantor_score::parse_score_file(&xml).expect("parses");
        eprintln!(
            "parts: {:?}",
            score
                .parts
                .iter()
                .map(|p| p.name.clone())
                .collect::<Vec<_>>()
        );
        assert!(score.parts.len() >= 4);
    }
}
