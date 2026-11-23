//! The three ONNX models (segnet, encoder, decoder) and their sessions.
use std::path::{Path, PathBuf};

/// File names of homr 0.7.1's models (see tools/omr/homr_port/fetch_models.sh).
pub const SEGNET: &str = "segnet_308-3296ccd40960f90ca6ab9c035cca945675d30a0f_fp16.onnx";
pub const ENCODER: &str = "encoder_pytorch_model_509-2293bba14bfea47d9d28411339362af15fce4c31.onnx";
pub const DECODER: &str = "decoder_pytorch_model_509-2293bba14bfea47d9d28411339362af15fce4c31.onnx";

pub struct Models {
    pub dir: PathBuf,
    pub segnet: ort::session::Session,
    pub encoder: ort::session::Session,
    pub decoder: ort::session::Session,
    /// Use CoreML for segnet and encoder (Apple only); the decoder always
    /// runs on the CPU in fp32 (CoreML fp16 changes its tokens).
    pub coreml: bool,
}

impl Models {
    /// Load the models from `dir`. `coreml`: run segnet and encoder on the
    /// Neural Engine (iOS/macOS), with the compiled models cached in `cache`.
    pub fn load(dir: &Path, coreml: bool, cache: &Path) -> Result<Self, crate::Error> {
        let _ = (dir, coreml, cache);
        todo!("sessions as in tools/omr/spike/omrprobe_lib.rs (PreferCPU for the decoder)")
    }
}
