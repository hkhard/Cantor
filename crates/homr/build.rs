// On iOS, ONNX Runtime is Microsoft's static xcframework (linked by ort-sys
// from ORT_IOS_XCFWK_PATH). A static archive does not bring its own
// dependencies: link the system libraries and frameworks it needs here.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("ios") {
        for l in ["z", "sqlite3", "c++"] {
            println!("cargo:rustc-link-lib=dylib={l}");
        }
        for f in ["CoreML", "Foundation", "Accelerate"] {
            println!("cargo:rustc-link-lib=framework={f}");
        }
    }
}
