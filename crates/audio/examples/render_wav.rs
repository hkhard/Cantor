//! Offline render of a score through the engine to a WAV file.
//! Usage: render_wav <score> <out.wav> [solo-channel|-] [seconds]
use cantor_audio::{Engine, SoundFontSynth};
use cantor_timeline::build_timemap;
use cantor_timeline::channels::{choir_from_title, plan_channels_for};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let path = &a[1];
    let s = cantor_score::parse_score_file(&std::fs::read(path).unwrap()).unwrap();
    let tm = build_timemap(&s);
    let title = std::path::Path::new(path)
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let plan = plan_channels_for(&s, &tm, choir_from_title(&title));
    let sf = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/sf2/GeneralUser-GS.sf2"
    );
    let sr = 44_100;
    let mut e = Engine::new(SoundFontSynth::open(sf.as_ref(), sr).unwrap(), &tm, &plan, sr);
    if let Some(solo) = a.get(3).and_then(|x| x.parse::<usize>().ok()) {
        e.mixer_mut().set_solo(solo, true);
    }
    let secs = a
        .get(4)
        .and_then(|x| x.parse::<f64>().ok())
        .unwrap_or(tm.duration_sec + 2.0);
    let n = (secs * sr as f64) as usize;
    let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
    e.play();
    let t = std::time::Instant::now();
    for (lc, rc) in l.chunks_mut(512).zip(r.chunks_mut(512)) {
        e.render(lc, rc);
    }
    let dt = t.elapsed().as_secs_f64();
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: sr,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&a[2], spec).unwrap();
    for i in 0..n {
        for x in [l[i], r[i]] {
            w.write_sample((x.clamp(-1.0, 1.0) * 32767.0) as i16).unwrap();
        }
    }
    w.finalize().unwrap();
    println!(
        "{}: {:.0} s in {:.2} s ({:.0}x realtime)",
        a[2],
        secs,
        dt,
        secs / dt
    );
    for (i, c) in plan.channels.iter().enumerate() {
        println!("  {i}: {:10} program {:3}", c.label, c.program);
    }
}
