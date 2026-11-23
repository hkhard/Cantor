//! The three ONNX models (segnet, encoder, decoder) and their sessions.
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ort::session::builder::AutoDevicePolicy;
use ort::session::Session;

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

/// ONNX Runtime allows one environment per process, so it lives for the
/// whole process and every `Models::load` shares it.
fn environment() -> Result<&'static ort::environment::Environment, crate::Error> {
    static ENV: OnceLock<ort::environment::Environment> = OnceLock::new();
    if let Some(env) = ENV.get() {
        return Ok(env);
    }
    let env = ort::init()
        .with_name("cantor-homr")
        .build()
        .map_err(model_err)?;
    Ok(ENV.get_or_init(|| env))
}

pub(crate) fn model_err(e: impl std::fmt::Display) -> crate::Error {
    crate::Error::Model(e.to_string())
}

/// One session. Without `coreml` the CPU is pinned: `ort` 2.x otherwise uses
/// `AutoDevicePolicy::MaxEfficiency`, which registers CoreML (fp16) on Apple
/// even when no provider is asked for, and that changes the decoder's tokens.
fn session(model: &Path, coreml: bool, cache: &Path) -> Result<Session, crate::Error> {
    if !model.exists() {
        return Err(crate::Error::Model(format!(
            "missing model {}",
            model.display()
        )));
    }
    let mut b = Session::builder(environment()?).map_err(model_err)?;
    if coreml {
        std::fs::create_dir_all(cache).map_err(model_err)?;
        b = b
            .with_execution_providers([ort::ep::CoreML::default()
                .with_model_format(ort::ep::coreml::ModelFormat::MLProgram)
                .with_compute_units(ort::ep::coreml::ComputeUnits::All)
                .with_model_cache_dir(cache.display())
                .build()
                .error_on_failure()])
            .map_err(model_err)?;
    } else {
        b = b
            .with_auto_device(AutoDevicePolicy::PreferCPU)
            .map_err(model_err)?;
    }
    b.commit_from_file(model).map_err(model_err)
}

impl Models {
    /// Load the models from `dir`. `coreml`: run segnet and encoder on the
    /// Neural Engine (iOS/macOS), with the compiled models cached in `cache`.
    pub fn load(dir: &Path, coreml: bool, cache: &Path) -> Result<Self, crate::Error> {
        Ok(Models {
            dir: dir.to_path_buf(),
            segnet: session(&dir.join(SEGNET), coreml, &cache.join("segnet"))?,
            encoder: session(&dir.join(ENCODER), coreml, &cache.join("encoder"))?,
            // The decoder has a dynamic KV-cache dimension that CoreML
            // MLProgram cannot take, and fp16 changes tokens: always CPU fp32.
            decoder: session(&dir.join(DECODER), false, cache)?,
            coreml,
        })
    }
}
