//! LIB-10 on Android: PDF pages for note recognition, rendered by Android's
//! PdfRenderer in `PdfPlugin.kt` (grey PNG files at 300 DPI, longest side
//! at most 4200 px, as `apple_convert::CgPdfPages` on iOS). The converter's
//! worker thread calls it; the plugin renders on its own thread.
#![cfg_attr(not(feature = "homr"), allow(dead_code))]

use crate::convert::PdfPages;
use std::path::PathBuf;

/// The Android PDF plugin, kept in the app state.
pub struct AndroidPdf(pub tauri::plugin::PluginHandle<tauri::Wry>);

/// Registers `PdfPlugin.kt`. Not fatal: without it, conversion reports an
/// error instead of the app failing to start.
pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("choirbird-pdf")
        .setup(|app, api| {
            use tauri::Manager;
            match api.register_android_plugin("se.nikielhard.choirbird", "PdfPlugin") {
                Ok(h) => {
                    app.manage(AndroidPdf(h));
                }
                Err(e) => log::info!("pdf: no Android PDF plugin ({e})"),
            }
            Ok(())
        })
        .build()
}

/// Renders through the plugin into `tmp` (cleared after each PDF).
pub struct AndroidPdfPages {
    pub app: tauri::AppHandle,
    pub tmp: PathBuf,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Args {
    pdf: String,
    out: String,
    dpi: f64,
    max_side: u32,
}

#[derive(serde::Deserialize)]
struct Rendered {
    files: Vec<String>,
}

impl PdfPages for AndroidPdfPages {
    fn pages(&self, pdf: &[u8]) -> Result<Vec<Vec<u8>>, String> {
        use tauri::Manager;
        let plugin = self
            .app
            .try_state::<AndroidPdf>()
            .ok_or("PDF renderer not loaded")?;
        let _ = std::fs::remove_dir_all(&self.tmp);
        std::fs::create_dir_all(&self.tmp).map_err(|e| e.to_string())?;
        // The queue hands over bytes; PdfRenderer wants a file.
        let src = self.tmp.join("in.pdf");
        std::fs::write(&src, pdf).map_err(|e| e.to_string())?;
        let r: Rendered = plugin
            .0
            .run_mobile_plugin(
                "render",
                Args {
                    pdf: src.to_string_lossy().into(),
                    out: self.tmp.to_string_lossy().into(),
                    dpi: 300.0,
                    max_side: 4200,
                },
            )
            .map_err(|e| e.to_string())?;
        let pages = r
            .files
            .iter()
            .map(|f| std::fs::read(f).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>();
        let _ = std::fs::remove_dir_all(&self.tmp);
        pages
    }
}
