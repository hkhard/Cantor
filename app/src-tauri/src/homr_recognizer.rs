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
