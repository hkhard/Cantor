use cantor_annotations::*;
fn layout(cols: usize, w: f32) -> Vec<Rect> {
    (0..20)
        .map(|m| {
            let per_page = cols * 4;
            Rect {
                page: (m / per_page) as u32,
                x: (m % cols) as f32 * w,
                y: ((m % per_page) / cols) as f32 * 0.2,
                w,
                h: 0.2,
            }
        })
        .collect()
}
#[test]
fn ann_04_measure_anchor_survives_reflow() {
    let a = layout(4, 0.25);
    let b = layout(3, 0.33);
    let m7 = a[7];
    let (px, py) = (m7.x + m7.w * 0.3, m7.y + m7.h * 0.6);
    let anchor = anchor_at(m7.page, px, py, &a);
    assert!(matches!(anchor, Anchor::Measure { measure: 7, .. }));
    let (p, x, y) = resolve(&anchor, &|m| b.get(m).copied()).unwrap();
    assert!(b[7].contains(p, x, y));
    assert_eq!(p, b[7].page);
    let (p, x, y) = resolve(&anchor, &|m| a.get(m).copied()).unwrap();
    assert_eq!(p, m7.page);
    assert!((x - px).abs() < 1e-5 && (y - py).abs() < 1e-5);
}
#[test]
fn ann_04_nearest_and_page_fallback() {
    let a = layout(4, 0.2);
    assert!(matches!(
        anchor_at(0, 0.95, 0.1, &a),
        Anchor::Measure { measure: 3, .. }
    ));
    assert!(matches!(
        anchor_at(9, 0.5, 0.5, &a),
        Anchor::Page { page: 9, .. }
    ));
    assert_eq!(
        resolve(
            &Anchor::Measure {
                measure: 99,
                staff: None,
                dx: 0.0,
                dy: 0.0
            },
            &|m| a.get(m).copied()
        ),
        None
    );
    assert_eq!(
        resolve(
            &Anchor::Page {
                page: 2,
                x: 0.1,
                y: 0.2
            },
            &|_| None
        ),
        Some((2, 0.1, 0.2))
    );
}
