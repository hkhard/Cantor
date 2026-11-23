//! `cantor-omr`: turn PDFs and forScore files into Cantor scores.
//! See [`cantor_omr::cli`] and `docs/guides/pdf-to-cantor.md`.

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = cantor_omr::cli::run(
        &args,
        cantor_omr::cli::Lang::from_env(),
        &mut std::io::stdout(),
    );
    std::process::ExitCode::from(code as u8)
}
