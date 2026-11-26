//! LIB-12: scan paper sheet music with the camera. The system scanner finds
//! the page edges and straightens the photos (iOS: VisionKit's
//! `VNDocumentCameraViewController`; Android: ML Kit's document scanner in
//! `ScanPlugin.kt`). Pages come back as JPEG bytes, and
//! `cantor_library::scan` turns them into one PDF score. Everything stays on
//! the device.

/// What a scan gave: JPEG pages, or nothing when the user cancelled.
pub type Pages = Option<Vec<Vec<u8>>>;

/// Standard base64 (with or without padding) to bytes; None on a bad digit.
/// (Android hands scanned pages and imported files over as base64.)
pub fn base64_decode(s: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    }
    let bytes: Vec<u8> = s
        .bytes()
        .filter(|c| !c.is_ascii_whitespace() && *c != b'=')
        .collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut acc = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            acc |= val(c)? << (18 - 6 * i);
        }
        let n = match chunk.len() {
            4 => 3,
            3 => 2,
            2 => 1,
            _ => return None,
        };
        out.extend_from_slice(&acc.to_be_bytes()[1..1 + n]);
    }
    Some(out)
}

#[cfg(target_os = "ios")]
mod ios {
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject, NSObject, NSObjectProtocol};
    use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
    use objc2_foundation::NSError;
    use objc2_ui_kit::{UIApplication, UIImage, UIViewController};
    use std::cell::RefCell;
    use std::sync::mpsc::Sender;

    pub type Reply = Sender<Result<super::Pages, String>>;

    pub fn available() -> bool {
        // VisionKit is linked (project.yml); `isSupported` is false without
        // a camera and in the simulator.
        AnyClass::get(c"VNDocumentCameraViewController")
            .map(|c| unsafe { msg_send![c, isSupported] })
            .unwrap_or(false)
    }

    pub struct Ivars {
        reply: RefCell<Option<Reply>>,
    }

    impl Ivars {
        fn send(&self, r: Result<super::Pages, String>) {
            if let Some(tx) = self.reply.borrow_mut().take() {
                let _ = tx.send(r);
            }
        }
    }

    /// VNDocumentCameraScan → JPEG pages (quality 0.8 keeps staff lines sharp).
    unsafe fn pages(scan: &AnyObject) -> Vec<Vec<u8>> {
        let n: usize = msg_send![scan, pageCount];
        (0..n)
            .filter_map(|i| {
                let img: Option<Retained<UIImage>> = msg_send![scan, imageOfPageAtIndex: i];
                img?.jpeg_representation(0.8).map(|d| d.to_vec())
            })
            .collect()
    }

    fn dismiss(controller: &AnyObject) {
        let none: *const AnyObject = std::ptr::null();
        let _: () =
            unsafe { msg_send![controller, dismissViewControllerAnimated: true, completion: none] };
    }

    define_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "ChoirBirdScanDelegate"]
        #[ivars = Ivars]
        pub struct ScanDelegate;

        unsafe impl NSObjectProtocol for ScanDelegate {}

        // VNDocumentCameraViewControllerDelegate, by selector (objc2 has no
        // VisionKit binding).
        impl ScanDelegate {
            #[unsafe(method(documentCameraViewController:didFinishWithScan:))]
            fn finished(&self, controller: &AnyObject, scan: &AnyObject) {
                let p = unsafe { pages(scan) };
                dismiss(controller);
                self.ivars().send(Ok((!p.is_empty()).then_some(p)));
            }

            #[unsafe(method(documentCameraViewControllerDidCancel:))]
            fn cancelled(&self, controller: &AnyObject) {
                dismiss(controller);
                self.ivars().send(Ok(None));
            }

            #[unsafe(method(documentCameraViewController:didFailWithError:))]
            fn failed(&self, controller: &AnyObject, error: &NSError) {
                dismiss(controller);
                self.ivars()
                    .send(Err(error.localizedDescription().to_string()));
            }
        }
    );

    thread_local! {
        // The camera holds its delegate weakly: keep it alive here.
        static DELEGATE: RefCell<Option<Retained<ScanDelegate>>> = const { RefCell::new(None) };
    }

    /// Present the document camera (main thread); the result goes to `reply`.
    pub fn present(reply: Reply) -> Result<(), String> {
        let mtm = MainThreadMarker::new().ok_or("scanner must run on the main thread")?;
        let cls = AnyClass::get(c"VNDocumentCameraViewController").ok_or("VisionKit missing")?;
        let delegate: Retained<ScanDelegate> = {
            let this = mtm.alloc::<ScanDelegate>().set_ivars(Ivars {
                reply: RefCell::new(Some(reply)),
            });
            unsafe { msg_send![super(this), init] }
        };
        let camera: Retained<UIViewController> = unsafe {
            let a: objc2::rc::Allocated<UIViewController> = msg_send![cls, alloc];
            msg_send![a, init]
        };
        let _: () = unsafe { msg_send![&camera, setDelegate: &*delegate] };
        let app = UIApplication::sharedApplication(mtm);
        #[allow(deprecated)]
        let window = app.keyWindow().ok_or("no window")?;
        let mut top = window.rootViewController().ok_or("no view controller")?;
        while let Some(next) = top.presentedViewController() {
            top = next;
        }
        top.presentViewController_animated_completion(&camera, true, None);
        DELEGATE.with(|d| *d.borrow_mut() = Some(delegate));
        Ok(())
    }
}

/// The Android scanner plugin (`ScanPlugin.kt`), kept in the app state.
#[cfg(target_os = "android")]
pub struct AndroidScanner(pub tauri::plugin::PluginHandle<tauri::Wry>);

/// Registers `ScanPlugin.kt` on Android; a no-op plugin elsewhere.
pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("choirbird-scan")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            {
                use tauri::Manager;
                // Not fatal: without the Kotlin plugin (ML Kit not built in,
                // see ROADMAP phase A) the app simply offers no scanner.
                match _api.register_android_plugin("se.nikielhard.choirbird", "ScanPlugin") {
                    Ok(h) => {
                        _app.manage(AndroidScanner(h));
                    }
                    Err(e) => log::info!("scan: no Android scanner plugin ({e})"),
                }
            }
            Ok(())
        })
        .build()
}

/// Whether this device can scan (a camera and the system scanner).
pub fn available(_app: &tauri::AppHandle) -> bool {
    #[cfg(target_os = "ios")]
    {
        ios::available()
    }
    #[cfg(target_os = "android")]
    {
        use tauri::Manager;
        #[derive(serde::Deserialize)]
        struct A {
            available: bool,
        }
        _app.try_state::<AndroidScanner>()
            .and_then(|s| s.0.run_mobile_plugin::<A>("available", ()).ok())
            .is_some_and(|a| a.available)
    }
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        false
    }
}

/// Show the system scanner and wait for the pages (blocking: call it off the
/// main thread).
pub fn scan(_app: &tauri::AppHandle) -> Result<Pages, String> {
    #[cfg(target_os = "ios")]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        let (etx, erx) = std::sync::mpsc::channel();
        _app.run_on_main_thread(move || {
            let _ = etx.send(ios::present(tx));
        })
        .map_err(|e| e.to_string())?;
        erx.recv().map_err(|e| e.to_string())??;
        rx.recv().map_err(|e| e.to_string())?
    }
    #[cfg(target_os = "android")]
    {
        use tauri::Manager;
        #[derive(serde::Deserialize)]
        struct R {
            pages: Vec<String>,
        }
        let s = _app
            .try_state::<AndroidScanner>()
            .ok_or("scanner not loaded")?;
        let r: R =
            s.0.run_mobile_plugin("scan", ())
                .map_err(|e| e.to_string())?;
        let pages = r
            .pages
            .iter()
            .map(|p| base64_decode(p).ok_or("bad page data"))
            .collect::<Result<Vec<_>, _>>()?;
        Ok((!pages.is_empty()).then_some(pages))
    }
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        Err("no camera scanner on this device".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lib_12_scanned_pages_arrive_as_base64() {
        assert_eq!(base64_decode("TWFu").unwrap(), b"Man");
        assert_eq!(base64_decode("TWE=").unwrap(), b"Ma");
        assert_eq!(base64_decode("TQ==").unwrap(), b"M");
        assert_eq!(base64_decode("/9j/4A==").unwrap(), [0xFF, 0xD8, 0xFF, 0xE0]);
        assert_eq!(base64_decode("").unwrap(), b"");
        assert!(base64_decode("T").is_none());
        assert!(base64_decode("TW!u").is_none());
    }
}
