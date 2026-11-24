//! Stage 5 against homr 0.7.1 on the Lacrymosa page: the dewarped 1280x256
//! staff images homr gives the transformer (fixtures/omr/lacrymosa-p1/
//! staff_{i}.png), from homr's own staffs (staffs.json) and the
//! preprocessed page, so this checks stage 5 alone.
use cantor_homr::{staff_images, Gray, Staff, System};

const DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/omr/lacrymosa-p1/"
);

#[derive(serde::Deserialize)]
struct Expected {
    multi: Vec<usize>,
    staffs: Vec<Staff>,
}

fn systems() -> Vec<System> {
    let e: Expected =
        serde_json::from_slice(&std::fs::read(format!("{DIR}staffs.json")).unwrap()).unwrap();
    let mut it = e.staffs.into_iter();
    e.multi
        .iter()
        .map(|&n| System {
            staffs: it.by_ref().take(n).collect(),
        })
        .collect()
}

#[test]
fn lib_06_homr_staff_images_match_python() {
    let page = Gray::from_png(&std::fs::read(format!("{DIR}preprocessed.png")).unwrap()).unwrap();
    let strips = staff_images::cut(&page, &systems());
    assert_eq!(strips.len(), 10);
    let mut worst = 0f64;
    for (i, s) in strips.iter().enumerate() {
        let want = Gray::from_png(&std::fs::read(format!("{DIR}staff_{i}.png")).unwrap()).unwrap();
        assert_eq!(
            (s.width, s.height),
            (want.width, want.height),
            "strip {i} size"
        );
        let diff: Vec<u32> = s
            .data
            .iter()
            .zip(&want.data)
            .map(|(a, b)| (*a as i32 - *b as i32).unsigned_abs())
            .collect();
        let mean = diff.iter().sum::<u32>() as f64 / diff.len() as f64;
        let off = diff.iter().filter(|&&d| d > 0).count();
        let big = diff.iter().filter(|&&d| d > 32).count();
        eprintln!("strip {i}: mean abs diff {mean:.4} grey levels, {off} px differ, {big} by more than 32");
        worst = worst.max(mean);
    }
    assert!(worst <= 2.0, "mean abs diff {worst} > 2 grey levels");
}

#[test]
fn lib_06_homr_staff_images_from_our_staffs_match_python() {
    // Stages 4 and 5 chained: our own staffs from the masks, then the strips.
    let mask = |n: &str| {
        let mut g = Gray::from_png(&std::fs::read(format!("{DIR}pred_{n}.png")).unwrap()).unwrap();
        for v in &mut g.data {
            *v = (*v > 0) as u8;
        }
        g
    };
    let pred = cantor_homr::Predictions {
        notehead: mask("notehead"),
        symbols: mask("symbols"),
        staff: mask("staff"),
        clefs_keys: mask("clefs_keys"),
        stems_rest: mask("stems_rest"),
    };
    let page = Gray::from_png(&std::fs::read(format!("{DIR}preprocessed.png")).unwrap()).unwrap();
    let systems = cantor_homr::staffs::detect(&pred, &page).unwrap();
    let strips = staff_images::cut(&page, &systems);
    assert_eq!(strips.len(), 10);
    for (i, s) in strips.iter().enumerate() {
        let want = Gray::from_png(&std::fs::read(format!("{DIR}staff_{i}.png")).unwrap()).unwrap();
        let sum: u64 = s
            .data
            .iter()
            .zip(&want.data)
            .map(|(a, b)| (*a as i64 - *b as i64).unsigned_abs())
            .sum();
        let mean = sum as f64 / s.data.len() as f64;
        assert!(mean <= 2.0, "strip {i}: mean abs diff {mean}");
    }
}
