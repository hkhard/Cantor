# homr on the iPad: spike code (2026-11-23)

Throwaway code that measured homr's three ONNX models on the iPad mini.
Kept for reproducing the numbers in `docs/omr-engine-comparison.md`. Not part
of the build.

- `capture.py`: runs homr in Python on one page image. It saves the real model
  inputs (segmentation patches, staff images for the encoder) and outputs.
  Run it with the homr venv's Python: `python capture.py page.png outdir`.
- `decref.py`: reference greedy decode of staff 0 with the fp32 CPU models,
  giving the token ids the native run must reproduce.
- `omrprobe_*`: a small Rust crate, built on `ort` 2.0.0-rc.15 and ONNX
  Runtime 1.31.1. It runs the segmentation net on 9 batches of 8 patches (one
  300 DPI page), then the encoder and the greedy decoder (with KV cache) on
  every staff. It times them, records peak memory, and checks the outputs
  against the Python reference.
  - Rename the files to `Cargo.toml`, `build.rs` and `src/lib.rs`.
  - On iOS, link ONNX Runtime from Microsoft's NuGet package
    (`Microsoft.ML.OnnxRuntime` 1.31.1, `runtimes/ios/native/onnxruntime.xcframework.zip`).
    Check the SHA-512 against the nuget.org catalog before use.
  - Set `ORT_IOS_XCFWK_PATH` in `.cargo/config.toml` `[env]`.
  - It was hooked into the app's `setup()` for the device run. The hook starts
    the probe when `Documents/omrprobe/go` exists and writes
    `Documents/omrprobe/result.txt`.

**Pitfall found:** `ort` 2.x defaults to `AutoDevicePolicy::MaxEfficiency`.
On Apple platforms this registers CoreML (fp16) even when no execution
provider is asked for. On the CPU path it changed the decoder's output:
62/117 tokens matched instead of 117/117. Pin `AutoDevicePolicy::PreferCPU`
unless CoreML is wanted.
