fn main() {
    // onnxruntime's static iOS framework needs these system libraries.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("ios") {
        for l in ["z", "sqlite3", "c++"] {
            println!("cargo:rustc-link-lib=dylib={l}");
        }
        for f in ["CoreML", "Foundation", "Accelerate"] {
            println!("cargo:rustc-link-lib=framework={f}");
        }
    }
}
