//! DATA-02: let the user choose a sync folder (iCloud Drive, OneDrive,
//! Google Drive, any Files location) with the system folder picker, and keep
//! access to it across launches through a security-scoped bookmark.

use std::path::PathBuf;

/// Where the bookmark of the chosen folder is kept (inside the app container,
/// not in Documents, so it is not shared or backed up as library content).
pub fn bookmark_path(user: &std::path::Path) -> PathBuf {
    user.join(".sync").join("folder.bookmark")
}

#[cfg(target_os = "ios")]
mod imp {
    use objc2::rc::Retained;
    use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
    use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
    use objc2_foundation::{
        NSArray, NSData, NSURLBookmarkCreationOptions, NSURLBookmarkResolutionOptions, NSURL,
    };
    use objc2_ui_kit::{UIApplication, UIDocumentPickerDelegate, UIDocumentPickerViewController};
    use objc2_uniform_type_identifiers::UTTypeFolder;
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::sync::mpsc::Sender;

    pub struct Ivars {
        reply: RefCell<Option<Sender<Option<Vec<u8>>>>>,
    }

    define_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "CantorFolderPickerDelegate"]
        #[ivars = Ivars]
        pub struct PickerDelegate;

        unsafe impl NSObjectProtocol for PickerDelegate {}

        unsafe impl UIDocumentPickerDelegate for PickerDelegate {
            #[unsafe(method(documentPicker:didPickDocumentsAtURLs:))]
            fn picked(&self, _picker: &UIDocumentPickerViewController, urls: &NSArray<NSURL>) {
                let bookmark = urls.firstObject().and_then(|url| unsafe {
                    url.startAccessingSecurityScopedResource();
                    let data = url
                        .bookmarkDataWithOptions_includingResourceValuesForKeys_relativeToURL_error(
                            NSURLBookmarkCreationOptions::empty(),
                            None,
                            None,
                        )
                        .ok();
                    url.stopAccessingSecurityScopedResource();
                    data.map(|d| d.to_vec())
                });
                if let Some(tx) = self.ivars().reply.borrow_mut().take() {
                    let _ = tx.send(bookmark);
                }
            }

            #[unsafe(method(documentPickerWasCancelled:))]
            fn cancelled(&self, _picker: &UIDocumentPickerViewController) {
                if let Some(tx) = self.ivars().reply.borrow_mut().take() {
                    let _ = tx.send(None);
                }
            }
        }
    );

    thread_local! {
        // The picker holds its delegate weakly: keep it alive here.
        static DELEGATE: RefCell<Option<Retained<PickerDelegate>>> = const { RefCell::new(None) };
    }

    /// Show the folder picker; the bookmark (or None if cancelled) is sent on `reply`.
    pub fn present(reply: Sender<Option<Vec<u8>>>) -> Result<(), String> {
        let mtm = MainThreadMarker::new().ok_or("picker must run on the main thread")?;
        let delegate: Retained<PickerDelegate> = {
            let this = mtm.alloc::<PickerDelegate>().set_ivars(Ivars {
                reply: RefCell::new(Some(reply)),
            });
            unsafe { msg_send![super(this), init] }
        };
        let types = NSArray::from_slice(&[unsafe { UTTypeFolder }]);
        let picker = UIDocumentPickerViewController::initForOpeningContentTypes(
            mtm.alloc::<UIDocumentPickerViewController>(),
            &types,
        );
        picker.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        let app = UIApplication::sharedApplication(mtm);
        #[allow(deprecated)]
        let window = app.keyWindow().ok_or("no window")?;
        let mut top = window.rootViewController().ok_or("no view controller")?;
        while let Some(next) = top.presentedViewController() {
            top = next;
        }
        top.presentViewController_animated_completion(&picker, true, None);
        DELEGATE.with(|d| *d.borrow_mut() = Some(delegate));
        Ok(())
    }

    /// Resolve a bookmark to a path and start security-scoped access (kept
    /// for the life of the process).
    pub fn resolve(bookmark: &[u8]) -> Option<PathBuf> {
        unsafe {
            let data = NSData::with_bytes(bookmark);
            let url =
                NSURL::URLByResolvingBookmarkData_options_relativeToURL_bookmarkDataIsStale_error(
                    &data,
                    NSURLBookmarkResolutionOptions::empty(),
                    None,
                    std::ptr::null_mut(),
                )
                .ok()?;
            url.startAccessingSecurityScopedResource();
            url.path().map(|p| PathBuf::from(p.to_string()))
        }
    }
}

#[cfg(target_os = "ios")]
pub use imp::{present, resolve};

/// Desktop: no picker (sync folder is set by path); bookmarks are the path.
#[cfg(not(target_os = "ios"))]
pub fn resolve(bookmark: &[u8]) -> Option<PathBuf> {
    Some(PathBuf::from(String::from_utf8_lossy(bookmark).to_string()))
}
