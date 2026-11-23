//! Apple glue for on-device conversion (LIB-10): render PDF pages to grey
//! PNG images with Core Graphics (homr wants 300 DPI-like pages), and post a
//! local notification when a conversion finishes. Compiled for iOS and macOS
//! (so the renderer is tested on the Mac). Used by builds with the `homr`
//! feature; without it only the tests use it.
#![cfg_attr(not(feature = "homr"), allow(dead_code))]

use crate::convert::PdfPages;
use objc2_core_foundation::{CFData, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGBitmapContextGetData, CGColorSpace, CGContext, CGDataProvider,
    CGImageAlphaInfo, CGPDFBox, CGPDFDocument, CGPDFPage,
};

/// Renders PDF pages at `dpi` to 8-bit grey PNGs, white background.
pub struct CgPdfPages {
    pub dpi: f64,
    /// Longest side limit in pixels, to bound memory on large pages.
    pub max_side: usize,
}

impl Default for CgPdfPages {
    fn default() -> Self {
        CgPdfPages {
            dpi: 300.0,
            max_side: 4200,
        }
    }
}

impl PdfPages for CgPdfPages {
    fn pages(&self, pdf: &[u8]) -> Result<Vec<Vec<u8>>, String> {
        let data = CFData::from_bytes(pdf);
        let provider = CGDataProvider::with_cf_data(Some(&data)).ok_or("cannot read the PDF")?;
        let doc = CGPDFDocument::with_provider(Some(&provider)).ok_or("not a PDF file")?;
        let n = CGPDFDocument::number_of_pages(Some(&doc));
        let mut out = Vec::with_capacity(n);
        for i in 1..=n {
            let page =
                CGPDFDocument::page(Some(&doc), i).ok_or_else(|| format!("page {i} missing"))?;
            out.push(self.render(&page)?);
        }
        Ok(out)
    }
}

impl CgPdfPages {
    fn render(&self, page: &CGPDFPage) -> Result<Vec<u8>, String> {
        let r = CGPDFPage::box_rect(Some(page), CGPDFBox::CropBox);
        let mut scale = self.dpi / 72.0;
        let longest = r.size.width.max(r.size.height) * scale;
        if longest > self.max_side as f64 {
            scale *= self.max_side as f64 / longest;
        }
        let w = (r.size.width * scale).round().max(1.0) as usize;
        let h = (r.size.height * scale).round().max(1.0) as usize;
        let space = CGColorSpace::new_device_gray().ok_or("no grey colour space")?;
        let mut buf = vec![255u8; w * h];
        // SAFETY: buf outlives the context, which is dropped at the end of
        // this function; rows are w bytes (8-bit grey, no alpha).
        let ctx = unsafe {
            CGBitmapContextCreate(
                buf.as_mut_ptr().cast(),
                w,
                h,
                8,
                w,
                Some(&space),
                CGImageAlphaInfo::None.0,
            )
        }
        .ok_or("cannot make a bitmap")?;
        CGContext::set_gray_fill_color(Some(&ctx), 1.0, 1.0);
        CGContext::fill_rect(
            Some(&ctx),
            CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(w as f64, h as f64)),
        );
        CGContext::scale_ctm(Some(&ctx), scale, scale);
        CGContext::translate_ctm(Some(&ctx), -r.origin.x, -r.origin.y);
        CGContext::draw_pdf_page(Some(&ctx), Some(page));
        let _ = CGBitmapContextGetData(Some(&ctx));
        drop(ctx);
        let mut png = Vec::new();
        image::GrayImage::from_raw(w as u32, h as u32, buf)
            .ok_or("bitmap size")?
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        Ok(png)
    }
}

/// Post a local notification now ("Hymn is ready to play"). Asks for
/// permission the first time; if the user says no, nothing is shown (the
/// in-app notice still appears).
#[cfg(target_os = "ios")]
pub fn notify(title: &str, body: &str) {
    use objc2_foundation::NSString;
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest,
        UNNotificationSound, UNUserNotificationCenter,
    };
    let center = UNUserNotificationCenter::currentNotificationCenter();
    let ask =
        block2::RcBlock::new(|_ok: objc2::runtime::Bool, _err: *mut objc2_foundation::NSError| {});
    center.requestAuthorizationWithOptions_completionHandler(
        UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
        &ask,
    );
    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(title));
    content.setBody(&NSString::from_str(body));
    content.setSound(Some(&UNNotificationSound::defaultSound()));
    let id = NSString::from_str(&format!("choirbird-convert-{title}"));
    let req = UNNotificationRequest::requestWithIdentifier_content_trigger(&id, &content, None);
    center.addNotificationRequest_withCompletionHandler(&req, None);
}

#[cfg(not(target_os = "ios"))]
pub fn notify(_title: &str, _body: &str) {}

#[cfg(test)]
mod lib_10_render {
    use super::*;

    #[test]
    fn lib_10_pdf_pages_render_to_grey_pngs_at_300_dpi() {
        let pdf = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/scores/mozart-lacrymosa/mozart-lacrymosa-from-requiem-satb-with-piano.pdf"
        ))
        .unwrap();
        let pages = CgPdfPages::default().pages(&pdf).unwrap();
        assert_eq!(pages.len(), 5);
        let img = image::load_from_memory(&pages[0]).unwrap().to_luma8();
        // 300 DPI: the page is 2700 x 3600 pt-scaled (a 9 x 12 in score page).
        let r = (img.height() as f64) / (img.width() as f64);
        assert!((1.2..1.5).contains(&r), "aspect {r}");
        assert!((3200..=3700).contains(&img.height()), "{}", img.height());
        // Mostly white paper with black ink, not inverted or blank.
        let dark = img.pixels().filter(|p| p.0[0] < 100).count() as f64
            / (img.width() * img.height()) as f64;
        assert!(dark > 0.01 && dark < 0.25, "dark share {dark}");
        assert_eq!(img.get_pixel(5, 5).0[0], 255, "corner is white");
        // Not upside down: the title is near the top (more ink in the top fifth than the bottom fifth).
        let h = img.height();
        let ink = |y0: u32, y1: u32| {
            (y0..y1)
                .flat_map(|y| (0..img.width()).map(move |x| (x, y)))
                .filter(|&(x, y)| img.get_pixel(x, y).0[0] < 100)
                .count()
        };
        assert!(
            ink(0, h / 5) > 0 && ink(0, h / 5) * 3 > ink(4 * h / 5, h),
            "orientation"
        );
    }

    #[test]
    fn lib_10_bad_pdf_is_an_error() {
        assert!(CgPdfPages::default().pages(b"not a pdf").is_err());
    }
}
