//! `cargo run -p cantor-score --example layout_boxes -- score.mxl` prints the
//! measure boxes from the file's layout as JSON (the app's measure map format).
fn main() {
    let path = std::env::args().nth(1).expect("score file");
    let bytes = std::fs::read(&path).expect("read");
    match cantor_score::layout_measure_boxes(&bytes) {
        Some(b) => {
            let items: Vec<String> = b
                .iter()
                .map(|b| {
                    format!(
                        "{{\"page\":{},\"x\":{},\"y\":{},\"w\":{},\"h\":{}}}",
                        b.page, b.x, b.y, b.w, b.h
                    )
                })
                .collect();
            println!("[{}]", items.join(","));
        }
        None => {
            eprintln!("no layout");
            std::process::exit(1)
        }
    }
}
