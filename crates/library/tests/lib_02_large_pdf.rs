//! LIB-02: a 500-page PDF imports in < 3 s. The PDF is built in the test
//! (valid structure: catalog, page tree, 500 pages each with its own content
//! stream of about 20 KB of drawing operators, cross-reference table), about
//! 10 MB in all, like a scanned-and-vectorised choir book. Import is timed
//! from bytes in memory to the score being listed in the library.
//! Thumbnails are generated lazily by the app, so they are not part of this.

use cantor_library::import::{import_file, list_library};
use std::time::Instant;

const PAGES: usize = 500;

fn big_pdf(pages: usize) -> Vec<u8> {
    let mut out: Vec<u8> = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    let obj = |out: &mut Vec<u8>, offsets: &mut Vec<usize>, body: &[u8]| {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", offsets.len()).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    };
    // 1: catalog, 2: page tree, then (page, content) pairs.
    obj(&mut out, &mut offsets, b"<< /Type /Catalog /Pages 2 0 R >>");
    let kids: Vec<String> = (0..pages).map(|i| format!("{} 0 R", 3 + 2 * i)).collect();
    obj(
        &mut out,
        &mut offsets,
        format!(
            "<< /Type /Pages /Count {pages} /Kids [{}] >>",
            kids.join(" ")
        )
        .as_bytes(),
    );
    for i in 0..pages {
        let content_id = 4 + 2 * i;
        obj(
            &mut out,
            &mut offsets,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents {content_id} 0 R >>"
            )
            .as_bytes(),
        );
        // Staff lines and note heads: ~20 KB per page.
        let mut s = String::new();
        for staff in 0..12 {
            for line in 0..5 {
                let y = 780 - staff * 62 - line * 6;
                s.push_str(&format!("40 {y} m 555 {y} l S\n"));
            }
            for n in 0..40 {
                let x = 50 + n * 12;
                let y = 760 - staff * 62 + (n * 7 + i) % 24;
                s.push_str(&format!(
                    "{x} {y} m {} {} {} {} {} {y} c f\n",
                    x + 2,
                    y + 3,
                    x + 6,
                    y + 3,
                    x + 8
                ));
            }
        }
        obj(
            &mut out,
            &mut offsets,
            format!("<< /Length {} >>\nstream\n{s}\nendstream", s.len() + 1).as_bytes(),
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
    out
}

#[test]
fn lib_02_five_hundred_page_pdf_imports_in_under_3_s() {
    let pdf = big_pdf(PAGES);
    // Sanity: the file really has 500 page objects.
    let text = String::from_utf8_lossy(&pdf);
    assert_eq!(text.matches("/Type /Page ").count(), PAGES);
    assert!(pdf.len() > 5_000_000, "{} bytes", pdf.len());

    let dir = std::env::temp_dir().join(format!("cantor-lib02-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let t = Instant::now();
    let got = import_file(&dir, "Choir book.pdf", &pdf).unwrap();
    let lib = list_library(&[dir.as_path()]);
    let took = t.elapsed();
    println!(
        "LIB-02 {PAGES} pages, {:.1} MB: imported and listed in {:.1} ms",
        pdf.len() as f64 / 1e6,
        took.as_secs_f64() * 1e3
    );
    assert_eq!(got.len(), 1);
    assert!(got[0].pdf && !got[0].playable);
    assert_eq!(lib.len(), 1);
    assert_eq!(lib[0].pdf_file.as_deref(), Some("Choir book.pdf"));
    assert_eq!(std::fs::read(dir.join("Choir book.pdf")).unwrap(), pdf);
    assert!(took.as_secs_f64() < 3.0, "{took:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
