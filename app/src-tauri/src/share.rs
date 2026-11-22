//! Native iOS share sheet (UIActivityViewController) for exported files:
//! rehearsal tracks go to AirDrop, Messages, Mail, Files and other apps.
//!
//! On iPad the sheet must be a popover anchored to a view; we anchor it to
//! the bottom centre of the key window, where the transport bar is.

#[cfg(target_os = "ios")]
pub fn share_files(paths: &[std::path::PathBuf]) -> Result<(), String> {
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::MainThreadMarker;
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_foundation::{NSArray, NSString, NSURL};
    use objc2_ui_kit::{UIActivityViewController, UIApplication};

    let mtm = MainThreadMarker::new().ok_or("share must run on the main thread")?;
    let urls: Vec<Retained<AnyObject>> = paths
        .iter()
        .map(|p| {
            let u = NSURL::fileURLWithPath(&NSString::from_str(&p.to_string_lossy()));
            Retained::into_super(Retained::into_super(u))
        })
        .collect();
    let items = NSArray::from_retained_slice(&urls);
    let app = UIApplication::sharedApplication(mtm);
    #[allow(deprecated)]
    let window = app.keyWindow().ok_or("no window")?;
    let root = window.rootViewController().ok_or("no view controller")?;
    // Present from the top-most presented controller.
    let mut top = root;
    while let Some(next) = top.presentedViewController() {
        top = next;
    }
    unsafe {
        let vc = UIActivityViewController::initWithActivityItems_applicationActivities(
            mtm.alloc::<UIActivityViewController>(),
            &items,
            None,
        );
        if let Some(pop) = vc.popoverPresentationController() {
            let view = window.as_ref();
            pop.setSourceView(Some(view));
            let b = view.bounds();
            pop.setSourceRect(CGRect::new(
                CGPoint::new(b.size.width / 2.0, b.size.height - 60.0),
                CGSize::new(1.0, 1.0),
            ));
        }
        top.presentViewController_animated_completion(&vc, true, None);
    }
    Ok(())
}

#[cfg(not(target_os = "ios"))]
pub fn share_files(paths: &[std::path::PathBuf]) -> Result<(), String> {
    // Desktop: reveal the files in the file manager.
    if let Some(dir) = paths.first().and_then(|p| p.parent()) {
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(dir).spawn();
        let _ = dir;
    }
    Ok(())
}
