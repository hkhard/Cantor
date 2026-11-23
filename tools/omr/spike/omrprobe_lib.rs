//! Spike: run homr's three ONNX models (segmentation, encoder, decoder) on
//! inputs captured from a real page, time them and report peak memory.
//! Checks the outputs against the Python reference (same page on the Mac).

use ort::session::Session;
use ort::value::Tensor;
use std::path::Path;
use std::time::Instant;

pub struct Report {
    pub lines: Vec<String>,
}

fn session(env: &ort::environment::Environment, model: &Path, coreml: bool, cache: &Path) -> ort::Result<Session> {
    // ort defaults to AutoDevicePolicy::MaxEfficiency, which on Apple
    // registers CoreML (fp16 on the Neural Engine) even when no provider is
    // asked for. Pin the CPU unless CoreML is wanted.
    // iPad mini (A17 Pro): 2 performance + 4 efficiency cores.
    let threads = std::env::var("PROBE_THREADS").ok().and_then(|t| t.parse().ok()).unwrap_or(4);
    let mut b = Session::builder(env)?.with_intra_threads(threads)?;
    if !coreml {
        b = b.with_auto_device(ort::session::builder::AutoDevicePolicy::PreferCPU)?;
    }
    if std::env::var("PROBE_OPT").as_deref() == Ok("0") {
        b = b.with_optimization_level(ort::session::builder::GraphOptimizationLevel::Disable)?;
    } else if std::env::var("PROBE_OPT").as_deref() == Ok("1") {
        b = b.with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level1)?;
    }
    if coreml {
        b = b.with_execution_providers([ort::ep::CoreML::default()
            .with_model_format(ort::ep::coreml::ModelFormat::MLProgram)
            .with_compute_units(ort::ep::coreml::ComputeUnits::All)
            .with_model_cache_dir(cache.display())
            .build()
            .error_on_failure()])?;
    }
    b.commit_from_file(model)
}

fn read(p: &Path) -> Vec<u8> {
    std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// `dir` holds the models (segnet*.onnx, encoder*.onnx, decoder*.onnx) and
/// the captured inputs (seg_in.f16, seg_ref.u8, enc_in_N.f32, dec_ref.json).
pub fn run(dir: &Path, coreml: bool, mem: &dyn Fn() -> f64) -> Report {
    let mut r = Report { lines: vec![] };
    let mut log = |s: String| {
        eprintln!("{s}");
        r.lines.push(s)
    };
    let find = |p: &str| {
        std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|f| f.file_name().unwrap().to_string_lossy().starts_with(p) && f.extension().is_some_and(|x| x == "onnx"))
            .unwrap_or_else(|| panic!("no {p} model"))
    };
    let env = ort::init().with_name("omrprobe").build().expect("ort env");
    let cache = dir.join("coreml-cache");
    let _ = std::fs::create_dir_all(&cache);
    log(format!("provider {} | start {:.0} MB", if coreml { "CoreML" } else { "CPU" }, mem()));
    let mut peak = mem();
    let mut note = |peak: &mut f64| {
        let m = mem();
        if m > *peak {
            *peak = m
        }
        m
    };

    // 1. Segmentation: 8 patches of 3x320x320 (fp16) per batch.
    let t = Instant::now();
    let mut seg = match session(&env, &find("segnet"), coreml, &cache) {
        Ok(s) => s,
        Err(e) => {
            log(format!("segnet load failed: {e}"));
            return r;
        }
    };
    log(format!("segnet load {:.2} s, {:.0} MB", t.elapsed().as_secs_f64(), note(&mut peak)));
    let raw = read(&dir.join("seg_in.f16"));
    let x: Vec<half::f16> = raw.chunks_exact(2).map(|c| half::f16::from_le_bytes([c[0], c[1]])).collect();
    let refm = read(&dir.join("seg_ref.u8"));
    let mut seg_ms = vec![];
    let mut agree = 0.0;
    // A 300 DPI A4 page needs about 70 patches (~9 batches); run 9.
    for i in 0..9 {
        let t = Instant::now();
        let input = Tensor::from_array(([8usize, 3, 320, 320], x.clone())).unwrap();
        let out = seg.run(ort::inputs!["input" => input]).unwrap();
        let (_, y) = out["output"].try_extract_tensor::<half::f16>().unwrap();
        if i == 0 {
            // argmax over the 6 classes, compare with Python's argmax.
            let plane = 320 * 320;
            let mut same = 0usize;
            for b in 0..8 {
                for p in 0..plane {
                    let mut best = 0;
                    for c in 1..6 {
                        if y[(b * 6 + c) * plane + p] > y[(b * 6 + best) * plane + p] {
                            best = c;
                        }
                    }
                    same += (best as u8 == refm[b * plane + p]) as usize;
                }
            }
            agree = same as f64 / (8 * plane) as f64;
        }
        seg_ms.push(t.elapsed().as_secs_f64());
        note(&mut peak);
    }
    log(format!(
        "segnet 9 batches: first {:.2} s, then {:.2} s each, argmax agrees with Python {:.2}%, {:.0} MB",
        seg_ms[0],
        seg_ms[1..].iter().sum::<f64>() / 8.0,
        agree * 100.0,
        note(&mut peak)
    ));
    drop(seg);

    // 2+3. Encoder + greedy decoder per staff.
    let t = Instant::now();
    let enc = session(&env, &find("encoder"), coreml, &cache);
    let dec = session(&env, &find("decoder"), false, &cache); // decoder: dynamic shapes, CPU
    let (mut enc, mut dec) = match (enc, dec) {
        (Ok(a), Ok(b)) => (a, b),
        (a, b) => {
            log(format!("transformer load failed: {:?} {:?}", a.err(), b.err()));
            return r;
        }
    };
    log(format!("encoder+decoder load {:.2} s, {:.0} MB", t.elapsed().as_secs_f64(), note(&mut peak)));
    let reference: Vec<[i64; 2]> = {
        let s = String::from_utf8(read(&dir.join("dec_ref.json"))).unwrap();
        s.trim_matches(['[', ']'])
            .split("], [")
            .map(|p| {
                let v: Vec<i64> = p.split(", ").map(|n| n.trim_matches(['[', ']']).parse().unwrap()).collect();
                [v[0], v[1]]
            })
            .collect()
    };
    let mut staff = 0;
    let (mut te, mut td, mut steps_all) = (0.0, 0.0, 0);
    while dir.join(format!("enc_in_{staff}.f32")).exists() {
        let raw = read(&dir.join(format!("enc_in_{staff}.f32")));
        let x: Vec<f32> = raw.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        let t = Instant::now();
        let out = enc.run(ort::inputs!["input" => Tensor::from_array(([1usize, 1, 256, 1280], x)).unwrap()]).unwrap();
        let (_, ctx) = out["output"].try_extract_tensor::<f32>().unwrap();
        let ctx: Vec<f32> = ctx.to_vec();
        if staff == 0 && std::env::var("PROBE_DEBUG").is_ok() {
            eprintln!("ctx sum {:.4} first {:?}", ctx.iter().sum::<f32>(), &ctx[..3]);
        }
        drop(out);
        te += t.elapsed().as_secs_f64();
        let t = Instant::now();
        let mut cache: Vec<(Vec<usize>, Vec<f32>)> = (0..32).map(|_| (vec![1, 8, 0, 64], vec![])).collect();
        let mut last = [1i64, 0, 0, 0, 0];
        let mut ids = vec![];
        for step in 0..608usize {
            let ctxv = if step == 0 { ctx.clone() } else { ctx[..512].to_vec() };
            let clen = if step == 0 { 1280 } else { 1 };
            let mut inputs: Vec<(std::borrow::Cow<str>, ort::session::SessionInputValue)> = vec![];
            for (name, v) in ["rhythms", "pitchs", "lifts", "articulations", "slurs"].iter().zip(last) {
                inputs.push(((*name).into(), Tensor::from_array(([1usize, 1], vec![v])).unwrap().into()));
            }
            inputs.push(("context".into(), Tensor::from_array(([1usize, clen, 512], ctxv)).unwrap().into()));
            inputs.push(("cache_len".into(), Tensor::from_array(([1usize], vec![step as i64])).unwrap().into()));
            for (i, (shape, data)) in cache.iter().enumerate() {
                inputs.push((format!("cache_in{i}").into(), Tensor::from_array((shape.clone(), data.clone())).unwrap().into()));
            }
            let out = dec.run(inputs).unwrap();
            let am = |name: &str| {
                let (_, v) = out[name].try_extract_tensor::<f32>().unwrap();
                v.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).unwrap().0 as i64
            };
            let (rh, pi, li, ar, sl) = (am("out_rhythms"), am("out_pitchs"), am("out_lifts"), am("out_articulations"), am("out_slurs"));
            for (i, c) in cache.iter_mut().enumerate() {
                let (s, v) = out[format!("cache_out{i}").as_str()].try_extract_tensor::<f32>().unwrap();
                *c = (s.iter().map(|&d| d as usize).collect(), v.to_vec());
            }
            if staff == 0 && step < 10 && std::env::var("PROBE_DEBUG").is_ok() {
                let (_, v) = out["out_rhythms"].try_extract_tensor::<f32>().unwrap();
                let mut top: Vec<(usize, f32)> = v.iter().copied().enumerate().collect();
                top.sort_by(|a, b| b.1.total_cmp(&a.1));
                eprintln!("step {step} rhythm top {:?}", &top[..2]);
            }
            if rh == 2 {
                break;
            }
            ids.push([rh, pi]);
            last = [rh, pi, li, ar, sl];
        }
        td += t.elapsed().as_secs_f64();
        steps_all += ids.len();
        if staff == 0 {
            let same = ids.iter().zip(&reference).filter(|(a, b)| a == b).count();
            let first = ids.iter().zip(&reference).position(|(a, b)| a != b);
            if let Some(i) = first {
                log(format!("first difference at token {i}: got {:?} want {:?}", &ids[i..(i + 4).min(ids.len())], &reference[i..(i + 4).min(reference.len())]));
            }
            log(format!("staff 0: {} tokens, {} of {} match the Python reference", ids.len(), same, reference.len()));
        }
        note(&mut peak);
        staff += 1;
    }
    log(format!(
        "{staff} staffs: encoder {:.2} s, decoder {:.2} s ({} tokens, {:.1} ms/token), {:.0} MB",
        te,
        td,
        steps_all,
        td * 1000.0 / steps_all.max(1) as f64,
        note(&mut peak)
    ));
    log(format!("peak {:.0} MB", peak));
    r
}
