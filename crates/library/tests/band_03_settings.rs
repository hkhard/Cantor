//! BAND-03: the instrument key is a per-score setting that survives a restart
//! and defaults to concert pitch for libraries saved before it existed.

use cantor_library::store::{ScoreSettings, Store};

#[test]
fn band_03_instrument_key_saved_per_score_and_restored() {
    let dir = std::env::temp_dir().join(format!("cantor-band03-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = Store::open(&dir);
    s.settings_mut("1:Blues").instrument_key = "Bb".into();
    s.save().unwrap();
    let again = Store::open(&dir);
    assert_eq!(again.settings("1:Blues").unwrap().instrument_key, "Bb");
    assert!(again.settings("1:Other").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn band_03_old_settings_without_the_field_read_as_concert_pitch() {
    let old: ScoreSettings =
        serde_json::from_str(r#"{"transpose": 2, "concert_pitch": true}"#).unwrap();
    assert_eq!(old.instrument_key, "C");
    assert_eq!(old.transpose, 2);
    assert_eq!(ScoreSettings::default().instrument_key, "C");
}
