//! LIB-12: pages photographed by the system document scanner (VisionKit on
//! iOS, ML Kit on Android) become one PDF. Each page is a JPEG embedded as is
//! (`/DCTDecode`, no re-encoding), on a page with the photo's aspect ratio.
//! Platform-free: the scanners hand over JPEG bytes.

/// Width and height of a baseline or progressive JPEG, and its number of
/// colour components (1 grey, 3 colour, 4 CMYK), read from the SOF marker.
pub fn jpeg_info(b: &[u8]) -> Option<(u32, u32, u8)> {
    if b.len() < 4 || b[0] != 0xFF || b[1] != 0xD8 {
        return None;
    }
    let mut i = 2;
    while i + 4 <= b.len() {
        if b[i] != 0xFF {
            return None;
        }
        let marker = b[i + 1];
        // Fill bytes and standalone markers have no length.
        if marker == 0xFF {
            i += 1;
            continue;
        }
        if marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            i += 2;
            continue;
        }
        let len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
        // SOF0..SOF15 except DHT (C4), JPG (C8) and DAC (CC).
        if (0xC0..=0xCF).contains(&marker) && ![0xC4, 0xC8, 0xCC].contains(&marker) {
            let s = b.get(i + 4..i + 4 + 6)?;
            let h = u16::from_be_bytes([s[1], s[2]]) as u32;
            let w = u16::from_be_bytes([s[3], s[4]]) as u32;
            return (w > 0 && h > 0).then_some((w, h, s[5]));
        }
        i += 2 + len;
    }
    None
}

/// The page size in PDF points for a photo: the long side is A4's long side
/// (842 pt), so a phone photo of an A4 sheet prints at about A4.
pub fn page_size(w: u32, h: u32) -> (f64, f64) {
    let long = 842.0;
    if h >= w {
        (long * w as f64 / h as f64, long)
    } else {
        (long, long * h as f64 / w as f64)
    }
}

/// One PDF with one page per JPEG, in order. Errors name the bad page (1-based).
pub fn jpegs_to_pdf(pages: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    if pages.is_empty() {
        return Err("no pages".into());
    }
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets: Vec<usize> = Vec::new();
    // Object numbers: 1 catalog, 2 pages, then per page: page, image, content.
    let n = pages.len();
    let obj = |o: &mut Vec<u8>, offs: &mut Vec<usize>, body: &[u8]| {
        offs.push(o.len());
        o.extend_from_slice(format!("{} 0 obj\n", offs.len()).as_bytes());
        o.extend_from_slice(body);
        o.extend_from_slice(b"\nendobj\n");
    };
    obj(&mut out, &mut offsets, b"<< /Type /Catalog /Pages 2 0 R >>");
    let kids: Vec<String> = (0..n).map(|k| format!("{} 0 R", 3 + 3 * k)).collect();
    obj(
        &mut out,
        &mut offsets,
        format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")).as_bytes(),
    );
    for (k, jpg) in pages.iter().enumerate() {
        let (w, h, comps) =
            jpeg_info(jpg).ok_or_else(|| format!("page {}: not a JPEG image", k + 1))?;
        let cs = match comps {
            1 => "/DeviceGray",
            4 => "/DeviceCMYK",
            _ => "/DeviceRGB",
        };
        let (pw, ph) = page_size(w, h);
        let (img, content) = (4 + 3 * k, 5 + 3 * k);
        obj(
            &mut out,
            &mut offsets,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {pw:.2} {ph:.2}] \
                 /Resources << /XObject << /Im{k} {img} 0 R >> >> /Contents {content} 0 R >>"
            )
            .as_bytes(),
        );
        let mut body = format!(
            "<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace {cs} \
             /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
            jpg.len()
        )
        .into_bytes();
        body.extend_from_slice(jpg);
        body.extend_from_slice(b"\nendstream");
        obj(&mut out, &mut offsets, &body);
        let draw = format!("q {pw:.2} 0 0 {ph:.2} 0 0 cm /Im{k} Do Q");
        obj(
            &mut out,
            &mut offsets,
            format!("<< /Length {} >>\nstream\n{draw}\nendstream", draw.len()).as_bytes(),
        );
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).as_bytes(),
    );
    for o in &offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            offsets.len() + 1
        )
        .as_bytes(),
    );
    Ok(out)
}

/// The default name of a scan: "Scan 2026-11-26 18.30".
pub fn default_name(prefix: &str, date: &str, time: &str) -> String {
    format!("{prefix} {date} {}", time.replace(':', "."))
}

/// Write the scanned pages as `<name>.pdf` in the library folder `dir`. A
/// scan is always a new score: a taken name gets a number, it never pairs
/// with an existing MusicXML of the same name. Returns the stem.
pub fn save_scan(
    dir: &std::path::Path,
    name: &str,
    pages: &[Vec<u8>],
) -> Result<crate::import::Imported, String> {
    let pdf = jpegs_to_pdf(pages)?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let stem = crate::import::free_stem(dir, &crate::import::safe_stem(&format!("{name}.pdf")));
    crate::import::write_atomic(&dir.join(format!("{stem}.pdf")), &pdf)
        .map_err(|e| e.to_string())?;
    Ok(crate::import::Imported {
        stem,
        playable: false,
        pdf: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal JPEG header: SOI, an APP0, a DQT-like segment, SOF0 with
    /// the given size and components, then EOI. Enough for `jpeg_info` and
    /// for checking the PDF structure (no decoder reads it here).
    fn jpeg(w: u16, h: u16, comps: u8, sof: u8) -> Vec<u8> {
        let mut b = vec![0xFF, 0xD8];
        b.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x10]);
        b.extend_from_slice(b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0");
        b.extend_from_slice(&[0xFF, 0xC4, 0x00, 0x04, 0x00, 0x00]); // DHT, skipped
        b.extend_from_slice(&[0xFF, sof, 0x00, 8 + 3 * comps, 8]);
        b.extend_from_slice(&h.to_be_bytes());
        b.extend_from_slice(&w.to_be_bytes());
        b.push(comps);
        for c in 0..comps {
            b.extend_from_slice(&[c + 1, 0x11, 0]);
        }
        b.extend_from_slice(&[0xFF, 0xD9]);
        b
    }

    fn real(name: &str) -> Vec<u8> {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/scan")
            .join(name);
        std::fs::read(p).unwrap()
    }

    #[test]
    fn lib_12_real_photos_keep_their_size() {
        assert_eq!(jpeg_info(&real("page-portrait.jpg")), Some((20, 28, 3)));
        assert_eq!(jpeg_info(&real("page-landscape.jpg")), Some((28, 20, 3)));
        let pdf = jpegs_to_pdf(&[real("page-portrait.jpg"), real("page-landscape.jpg")]).unwrap();
        assert_eq!(
            lopdf::Document::load_mem(&pdf).unwrap().get_pages().len(),
            2
        );
    }

    #[test]
    fn lib_12_jpeg_size_is_read_from_the_frame_header() {
        assert_eq!(jpeg_info(&jpeg(3024, 4032, 3, 0xC0)), Some((3024, 4032, 3)));
        // Progressive (SOF2) and grey.
        assert_eq!(jpeg_info(&jpeg(800, 600, 1, 0xC2)), Some((800, 600, 1)));
        assert_eq!(jpeg_info(b"\x89PNG\r\n"), None);
        assert_eq!(jpeg_info(&[0xFF, 0xD8, 0xFF]), None);
    }

    #[test]
    fn lib_12_page_has_the_photos_aspect_ratio() {
        let (w, h) = page_size(3024, 4032);
        assert_eq!(h, 842.0);
        assert!((w / h - 0.75).abs() < 1e-9);
        let (w, h) = page_size(4032, 3024);
        assert_eq!(w, 842.0);
        assert!((h / w - 0.75).abs() < 1e-9);
    }

    #[test]
    fn lib_12_scanned_pages_become_one_pdf_in_order() {
        let pages = vec![jpeg(3024, 4032, 3, 0xC0), jpeg(4032, 3024, 1, 0xC0)];
        let pdf = jpegs_to_pdf(&pages).unwrap();
        let doc = lopdf::Document::load_mem(&pdf).expect("a valid PDF");
        let ids: Vec<_> = doc.get_pages().into_values().collect();
        assert_eq!(ids.len(), 2);
        let media = |id| {
            let p = doc.get_dictionary(id).unwrap();
            let a = p.get(b"MediaBox").unwrap().as_array().unwrap();
            (a[2].as_float().unwrap(), a[3].as_float().unwrap())
        };
        let (w0, h0) = media(ids[0]);
        assert!((w0 - 631.5).abs() < 0.01 && (h0 - 842.0).abs() < 0.01);
        let (w1, h1) = media(ids[1]);
        assert!((w1 - 842.0).abs() < 0.01 && (h1 - 631.5).abs() < 0.01);
        // Each JPEG is embedded unchanged, as DCTDecode.
        let imgs: Vec<_> = doc
            .objects
            .values()
            .filter_map(|o| o.as_stream().ok())
            .filter(|s| s.dict.get(b"Subtype").and_then(|v| v.as_name()).ok() == Some(b"Image"))
            .collect();
        assert_eq!(imgs.len(), 2);
        for p in &pages {
            let s = imgs
                .iter()
                .find(|s| &s.content == p)
                .expect("page embedded");
            assert_eq!(
                s.dict.get(b"Filter").unwrap().as_name().unwrap(),
                b"DCTDecode"
            );
        }
    }

    #[test]
    fn lib_12_no_pages_or_a_bad_page_is_an_error() {
        assert!(jpegs_to_pdf(&[]).is_err());
        let e = jpegs_to_pdf(&[jpeg(10, 10, 3, 0xC0), b"nope".to_vec()]).unwrap_err();
        assert!(e.contains("page 2"), "{e}");
    }

    #[test]
    fn lib_12_scan_is_saved_as_a_new_pdf_score() {
        let d = std::env::temp_dir().join(format!("cantor-lib12-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        // A MusicXML with the same name exists: the scan must not pair with it.
        std::fs::write(d.join("Ave verum.musicxml"), "<score-partwise/>").unwrap();
        // Real JPEGs (our own icon, made with `sips`): a malformed stand-in is
        // quarantined by some virus scanners while it is written.
        let pages = vec![real("page-portrait.jpg")];
        let a = save_scan(&d, "Ave verum", &pages).unwrap();
        assert_ne!(a.stem, "Ave verum");
        assert!(a.pdf && !a.playable);
        let b = save_scan(&d, "Ave verum", &pages).unwrap();
        assert_ne!(a.stem, b.stem);
        let bytes = std::fs::read(d.join(format!("{}.pdf", a.stem))).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
        // No pages: nothing written.
        let before = std::fs::read_dir(&d).unwrap().count();
        assert!(save_scan(&d, "Empty", &[]).is_err());
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), before);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn lib_12_default_name_has_date_and_time() {
        assert_eq!(
            default_name("Scan", "2026-11-26", "18:30"),
            "Scan 2026-11-26 18.30"
        );
    }
}
