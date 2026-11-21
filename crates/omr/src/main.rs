//! `cantor-omr`: turn PDFs and forScore files into MusicXML + MIDI.
//!
//! Usage: cantor-omr <out-dir> <file.pdf|file.4sc|file.4ss>...
//! Audiveris is found via $CANTOR_AUDIVERIS or the usual install paths.

use cantor_omr::{convert_with, Audiveris, Input, Preprocessor, ScriptCleaner};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: cantor-omr <out-dir> <file.pdf|file.4sc|file.4ss>...");
        return ExitCode::from(2);
    }
    let Some(engine) = Audiveris::locate() else {
        eprintln!("Audiveris not found: install it or set CANTOR_AUDIVERIS to its binary");
        return ExitCode::from(2);
    };
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cleaner = ScriptCleaner::locate(&repo_root);
    if cleaner.is_none() {
        eprintln!("note: scan cleanup not set up (tools/omr/README.md); hard scans may fail");
    }
    let out = PathBuf::from(&args[0]);
    if let Err(e) = std::fs::create_dir_all(&out) {
        eprintln!("{}: {e}", out.display());
        return ExitCode::FAILURE;
    }

    let (mut ok, mut failed) = (0, 0);
    for file in &args[1..] {
        let inputs = match std::fs::read(file)
            .map_err(Into::into)
            .and_then(|b| Input::from_file_bytes(file, &b))
        {
            Ok(i) => i,
            Err(e) => {
                eprintln!("{file}: {e}");
                failed += 1;
                continue;
            }
        };
        for input in inputs {
            let title = input.title().to_string();
            let t0 = std::time::Instant::now();
            match convert_with(
                &engine,
                cleaner.as_ref().map(|c| c as &dyn Preprocessor),
                input,
                &out,
            ) {
                Ok(c) => {
                    ok += 1;
                    println!(
                        "ok   {title}: {} parts, {} measures, {} notes, {:.0} s of music{} -> {} ({:.0} s)",
                        c.parts,
                        c.measures,
                        c.notes,
                        c.duration_sec,
                        if c.cleaned { ", after scan cleanup" } else { "" },
                        c.midi.display(),
                        t0.elapsed().as_secs_f64()
                    );
                }
                Err(e) => {
                    failed += 1;
                    println!("FAIL {title}: {e}");
                }
            }
        }
    }
    println!("{ok} converted, {failed} failed");
    if failed > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
