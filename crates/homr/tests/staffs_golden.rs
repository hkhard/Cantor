//! Stage 4 against homr 0.7.1 on the Lacrymosa page: staffs and systems from
//! the filtered masks (fixtures/omr/lacrymosa-p1/pred_*.png), compared with
//! homr's staffs.json (after `repair_systems`).
use cantor_homr::{staffs, Gray, Predictions, Staff};

const DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/omr/lacrymosa-p1/"
);

fn mask(name: &str) -> Gray {
    let mut g = Gray::from_png(&std::fs::read(format!("{DIR}pred_{name}.png")).unwrap()).unwrap();
    for v in &mut g.data {
        *v = (*v > 0) as u8;
    }
    g
}

#[derive(serde::Deserialize)]
struct Expected {
    multi: Vec<usize>,
    staffs: Vec<Staff>,
}

#[test]
fn lib_06_homr_staffs_match_python() {
    let pred = Predictions {
        notehead: mask("notehead"),
        symbols: mask("symbols"),
        staff: mask("staff"),
        clefs_keys: mask("clefs_keys"),
        stems_rest: mask("stems_rest"),
    };
    let image = Gray::from_png(&std::fs::read(format!("{DIR}preprocessed.png")).unwrap()).unwrap();
    let systems = staffs::detect(&pred, &image).unwrap();
    let want: Expected =
        serde_json::from_slice(&std::fs::read(format!("{DIR}staffs.json")).unwrap()).unwrap();

    let sizes: Vec<usize> = systems.iter().map(|s| s.staffs.len()).collect();
    assert_eq!(sizes, want.multi, "systems");
    let got: Vec<&Staff> = systems.iter().flat_map(|s| &s.staffs).collect();
    assert_eq!(got.len(), 10);
    assert_eq!(got.len(), want.staffs.len());
    let (mut worst_y, mut worst_x) = (0f64, 0f64);
    for (i, (g, w)) in got.iter().zip(&want.staffs).enumerate() {
        assert_eq!(g.grandstaff, w.grandstaff, "staff {i} grandstaff");
        assert_eq!(g.grid.len(), w.grid.len(), "staff {i} grid points");
        for (p, q) in g.grid.iter().zip(&w.grid) {
            assert_eq!(p.y.len(), q.y.len());
            worst_x = worst_x.max((p.x - q.x).abs());
            for (a, b) in p.y.iter().zip(&q.y) {
                worst_y = worst_y.max((a - b).abs());
            }
        }
        assert!((g.min_y - w.min_y).abs() <= 1.0 && (g.max_y - w.max_y).abs() <= 1.0);
        assert!((g.min_x - w.min_x).abs() <= 2.0 && (g.max_x - w.max_x).abs() <= 2.0);
    }
    eprintln!("staffs: worst line y difference {worst_y:.6} px, x {worst_x:.6} px");
    assert!(worst_y <= 1.0, "line ys within 1 px");
    assert!(worst_x <= 2.0, "x within 2 px");
}
