//! Android: the bundled resources (the demo score, the SoundFont) live inside
//! the APK as assets, not as files. The app reads them as ordinary files, so
//! they are copied once into the app's own storage, and again when the app
//! version changes (an update may bring new resources).
//!
//! The copy logic is platform-free and tested on the Mac; only the asset
//! reader is Android code.

use std::path::Path;

/// A source of bundled files: list a folder, read a file.
pub trait AssetSource {
    /// File names (not folders) directly inside `dir` ("" is the root).
    fn list(&self, dir: &str) -> Vec<String>;
    fn read(&self, path: &str) -> Option<Vec<u8>>;
}

/// Copy the listed files from `src` into `dest`, keeping their relative
/// paths, unless `dest/.version` already says `version`. `dirs` are asset
/// folders ("resources/scores"); `files` single assets
/// ("resources/GeneralUser-GS.sf2"). Returns how many files were written.
pub fn extract(
    src: &dyn AssetSource,
    dest: &Path,
    version: &str,
    dirs: &[&str],
    files: &[&str],
) -> std::io::Result<usize> {
    let stamp = dest.join(".version");
    // The stamp is the version plus the list of bundled files, so a build
    // that adds files (the homr models) without a version bump copies too.
    // Listing is cheap; reading the 123 MB of models is not.
    let mut names: Vec<String> = dirs
        .iter()
        .flat_map(|d| src.list(d).into_iter().map(move |f| format!("{d}/{f}")))
        .collect();
    names.extend(files.iter().map(|f| f.to_string()));
    let want = format!("{version}\n{}", names.join("\n"));
    if std::fs::read_to_string(&stamp).ok().as_deref() == Some(want.as_str()) {
        return Ok(0);
    }
    let mut n = 0;
    let mut write = |rel: &str| -> std::io::Result<()> {
        if let Some(bytes) = src.read(rel) {
            let out = dest.join(rel);
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p)?;
            }
            // Write beside, then rename: a half-copied SoundFont never
            // stays behind if the app is killed mid-copy.
            let tmp = out.with_extension("part");
            std::fs::write(&tmp, bytes)?;
            std::fs::rename(&tmp, &out)?;
            n += 1;
        }
        Ok(())
    };
    for d in dirs {
        for f in src.list(d) {
            write(&format!("{d}/{f}"))?;
        }
    }
    for f in files {
        write(f)?;
    }
    std::fs::create_dir_all(dest)?;
    std::fs::write(stamp, want)?;
    Ok(n)
}

#[cfg(target_os = "android")]
pub mod apk {
    use super::AssetSource;
    use std::ffi::CString;
    use std::io::Read;

    /// The APK's assets through the NDK AssetManager.
    pub struct Apk(ndk::asset::AssetManager);

    impl Apk {
        /// The app's AssetManager, from the Android context.
        pub fn open() -> Option<Self> {
            let ctx = ndk_context::android_context();
            let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.ok()?;
            let mut env = vm.attach_current_thread().ok()?;
            let context = unsafe { jni::objects::JObject::from_raw(ctx.context().cast()) };
            let assets = env
                .call_method(
                    &context,
                    "getAssets",
                    "()Landroid/content/res/AssetManager;",
                    &[],
                )
                .ok()?
                .l()
                .ok()?;
            let ptr = unsafe {
                ndk_sys::AAssetManager_fromJava(env.get_raw().cast(), assets.as_raw().cast())
            };
            // The Java AssetManager lives as long as the app; keep it alive.
            let _ = env.new_global_ref(&assets).map(std::mem::forget);
            std::ptr::NonNull::new(ptr)
                .map(|p| Apk(unsafe { ndk::asset::AssetManager::from_ptr(p) }))
        }
    }

    impl AssetSource for Apk {
        fn list(&self, dir: &str) -> Vec<String> {
            let Ok(c) = CString::new(dir) else {
                return vec![];
            };
            let Some(it) = self.0.open_dir(&c) else {
                return vec![];
            };
            it.map(|f| f.to_string_lossy().into_owned()).collect()
        }
        fn read(&self, path: &str) -> Option<Vec<u8>> {
            let c = CString::new(path).ok()?;
            let mut a = self.0.open(&c)?;
            let mut v = Vec::new();
            a.read_to_end(&mut v).ok()?;
            Some(v)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct Fake(BTreeMap<String, Vec<u8>>);
    impl AssetSource for Fake {
        fn list(&self, dir: &str) -> Vec<String> {
            let p = format!("{dir}/");
            self.0
                .keys()
                .filter_map(|k| k.strip_prefix(&p))
                .filter(|r| !r.contains('/'))
                .map(String::from)
                .collect()
        }
        fn read(&self, path: &str) -> Option<Vec<u8>> {
            self.0.get(path).cloned()
        }
    }

    fn fake() -> Fake {
        Fake(BTreeMap::from([
            ("resources/scores/a.mxl".into(), b"A".to_vec()),
            ("resources/scores/a.pdf".into(), b"P".to_vec()),
            ("resources/GeneralUser-GS.sf2".into(), b"SF".to_vec()),
            ("other/x".into(), b"X".to_vec()),
        ]))
    }

    #[test]
    fn and_03_a_build_with_new_assets_copies_them_at_the_same_version() {
        // A development build adds the homr models without a version bump:
        // they must still be copied.
        let d = std::env::temp_dir().join(format!("cantor-and03-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let dirs = ["resources/scores", "resources/homr"];
        let sf = ["resources/GeneralUser-GS.sf2"];
        assert_eq!(extract(&fake(), &d, "0.1.0", &dirs, &sf).unwrap(), 3);
        let mut with_models = fake();
        with_models
            .0
            .insert("resources/homr/segnet.onnx".into(), b"M".to_vec());
        assert_eq!(extract(&with_models, &d, "0.1.0", &dirs, &sf).unwrap(), 4);
        assert_eq!(
            std::fs::read(d.join("resources/homr/segnet.onnx")).unwrap(),
            b"M"
        );
        // Unchanged: nothing copied again.
        assert_eq!(extract(&with_models, &d, "0.1.0", &dirs, &sf).unwrap(), 0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn and_01_bundled_resources_are_copied_once_per_version() {
        let d = std::env::temp_dir().join(format!("cantor-and01-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let n = extract(
            &fake(),
            &d,
            "1.0.0",
            &["resources/scores"],
            &["resources/GeneralUser-GS.sf2"],
        )
        .unwrap();
        assert_eq!(n, 3);
        assert_eq!(
            std::fs::read(d.join("resources/scores/a.mxl")).unwrap(),
            b"A"
        );
        assert_eq!(
            std::fs::read(d.join("resources/GeneralUser-GS.sf2")).unwrap(),
            b"SF"
        );
        assert!(!d.join("other/x").exists());
        // Same version: nothing copied again.
        assert_eq!(
            extract(
                &fake(),
                &d,
                "1.0.0",
                &["resources/scores"],
                &["resources/GeneralUser-GS.sf2"]
            )
            .unwrap(),
            0
        );
        // An update: copied again.
        assert_eq!(
            extract(
                &fake(),
                &d,
                "1.0.1",
                &["resources/scores"],
                &["resources/GeneralUser-GS.sf2"]
            )
            .unwrap(),
            3
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
