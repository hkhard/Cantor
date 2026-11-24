//! Dev aid: run the port on one page image and print what each stage found,
//! to compare with `homr --debug` in Python.
//!   cargo run --release -p cantor-homr --example page_systems -- page.png [out.musicxml]
use cantor_homr::{boxes, musicxml, preprocess, segnet, staff_images, staffs, transformer, Models};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let png = std::fs::read(&args[1]).expect("page png");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut models =
        Models::load(&root.join("models/homr"), false, &std::env::temp_dir()).expect("models");
    let pre = preprocess::preprocess_png(&png).unwrap();
    let pred = boxes::filter_predictions(segnet::segment(&mut models, &pre.image).unwrap());
    let systems = staffs::detect(&pred, &pre.image).unwrap();
    println!(
        "systems: {:?}",
        systems
            .iter()
            .map(|s| s
                .staffs
                .iter()
                .map(|st| if st.grandstaff { "G" } else { "s" })
                .collect::<String>())
            .collect::<Vec<_>>()
    );
    for (i, s) in systems.iter().enumerate() {
        for st in &s.staffs {
            println!(
                "  system {i}: y {:.0}..{:.0} x {:.0}..{:.0} grand {}",
                st.min_y, st.max_y, st.min_x, st.max_x, st.grandstaff
            );
        }
    }
    if let Some(out) = args.get(2) {
        let strips = staff_images::cut(&pre.image, &systems);
        let tokens: Vec<_> = strips
            .iter()
            .map(|s| transformer::read_staff(&mut models, s).unwrap())
            .collect();
        std::fs::write(out, musicxml::generate(&systems, &tokens)).unwrap();
        println!("wrote {out}");
    }
}
