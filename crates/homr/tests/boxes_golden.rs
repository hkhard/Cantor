//! Stage 3 against homr 0.7.1 on the Lacrymosa page: the boxes homr's
//! `predict_symbols` makes from the five masks (fixtures/omr/lacrymosa-p1/
//! pred_*.png, already filtered). Expected values in boxes.json come from
//! tools/omr/homr_port/capture_boxes.py.
use cantor_homr::boxes::{self, AngledBox};
use cantor_homr::Gray;

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

fn expected(key: &str) -> Vec<[f64; 5]> {
    let v: serde_json::Value =
        serde_json::from_slice(&std::fs::read(format!("{DIR}boxes.json")).unwrap()).unwrap();
    serde_json::from_value(v[key].clone()).unwrap()
}

/// Compares boxes in order: centre and size within `tol` px, angle within 0.01 degrees.
fn compare(name: &str, got: &[AngledBox], want: &[[f64; 5]], tol: f64) {
    assert_eq!(got.len(), want.len(), "{name}: number of boxes");
    let mut worst = 0f64;
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        let r = &g.rect;
        let d = [r.cx - w[0], r.cy - w[1], r.w - w[2], r.h - w[3]]
            .iter()
            .fold(0f64, |a, b| a.max(b.abs()));
        worst = worst.max(d);
        assert!(
            d <= tol && (r.angle - w[4]).abs() <= 0.01,
            "{name}[{i}]: got {r:?}, want {w:?}"
        );
    }
    eprintln!(
        "{name}: {} boxes, worst difference {worst:.5} px",
        got.len()
    );
}

#[test]
fn lib_06_homr_boxes_match_python() {
    let notehead = mask("notehead");
    let staff = mask("staff");
    let clefs = mask("clefs_keys");
    let stems = mask("stems_rest");

    let heads = boxes::create_bounding_ellipses(&notehead, Some((4.0, 4.0)));
    compare("noteheads", &heads, &expected("noteheads"), 1e-3);

    let frags = boxes::create_rotated_bounding_boxes(
        &staff,
        true,
        Some((5.0, 1.0)),
        Some((10000.0, 100.0)),
    );
    compare(
        "staff_fragments",
        &frags,
        &expected("staff_fragments"),
        1e-3,
    );

    let ck = boxes::create_rotated_bounding_boxes(
        &clefs,
        false,
        Some((20.0, 40.0)),
        Some((1000.0, 1000.0)),
    );
    compare("clefs_keys", &ck, &expected("clefs_keys"), 1e-3);

    let sr = boxes::create_rotated_bounding_boxes(&stems, false, None, None);
    compare("stems_rest", &sr, &expected("stems_rest"), 1e-3);

    let bar_img = cantor_homr::staffs::prepare_bar_line_image(&stems);
    let bars = boxes::create_rotated_bounding_boxes(&bar_img, true, Some((1.0, 5.0)), None);
    compare("bar_lines", &bars, &expected("bar_lines"), 1e-3);
}

#[test]
fn lib_06_homr_filter_keeps_a_clean_page() {
    // The Lacrymosa staff mask has no noisy cells: filtering keeps every pixel.
    let staff = mask("staff");
    let mut s255 = staff.clone();
    for v in &mut s255.data {
        *v *= 255;
    }
    assert!(boxes::noise_mask(&s255).is_none());
}

#[test]
fn lib_06_homr_noise_grid_filters_a_noisy_half() {
    // Left half clean, right half salt-and-pepper: the right cells are cut.
    let (w, h) = (300, 200);
    let mut g = Gray::new(w, h);
    let mut seed = 12345u32;
    for y in 0..h {
        for x in 150..w {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            g.set(x, y, if (seed >> 16) & 1 == 1 { 255 } else { 0 });
        }
    }
    let m = boxes::noise_mask(&g).expect("noisy cells are filtered");
    // The clean half is kept; most noisy cells are cut (homr stores the
    // noise in a uint8 grid, so a few wrap below the limit and stay).
    assert!((0..h).all(|y| (0..150).all(|x| m.get(x, y) == 255)));
    let cut = (0..h)
        .flat_map(|y| (150..w).map(move |x| (x, y)))
        .filter(|&(x, y)| m.get(x, y) == 0)
        .count();
    assert!(cut > 150 * h / 2, "{cut} noisy pixels cut");
}
