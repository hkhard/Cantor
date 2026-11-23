//! `cargo run -p cantor-ocr --example read_title -- page1.png ...`:
//! print the recognised lines and the guessed metadata of each image.
fn main() {
    for path in std::env::args().skip(1) {
        let png = std::fs::read(&path).expect("read image");
        let t = std::time::Instant::now();
        match cantor_ocr::recognize(&png, &["sv-SE", "en-US"]) {
            Ok(lines) => {
                let m = cantor_ocr::guess_metadata(&lines);
                println!(
                    "== {path} ({} lines, {} ms)",
                    lines.len(),
                    t.elapsed().as_millis()
                );
                for l in lines.iter().filter(|l| l.y < 0.35).take(12) {
                    println!("   {:.3} {:.3} h{:.3}  {}", l.x, l.y, l.h, l.text);
                }
                println!("   -> {m:?}");
            }
            Err(e) => println!("== {path}: {e}"),
        }
    }
}
