use cantor_annotations::*;
#[test]
fn ann_03_stamps_scale_with_zoom() {
    let s = Stamp {
        kind: StampKind::Mf,
        scale: 0.03,
    };
    assert_eq!(s.pixel_size(1000.0), 30.0);
    assert_eq!(s.pixel_size(2000.0), 60.0);
    let it = Item::Stamp(s);
    let small = to_svg(
        0,
        &[Placed {
            x: 0.5,
            y: 0.5,
            item: &it,
        }],
        500.0,
        1000.0,
    );
    let big = to_svg(
        0,
        &[Placed {
            x: 0.5,
            y: 0.5,
            item: &it,
        }],
        1000.0,
        2000.0,
    );
    assert!(small.contains("font-size=\"30\""), "{small}");
    assert!(big.contains("font-size=\"60\""), "{big}");
    assert_eq!(StampKind::Breath.glyph(), "∨");
    assert_eq!(StampKind::Fingering(3).glyph(), "3");
}
