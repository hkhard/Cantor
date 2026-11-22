//! Vocal rendering providers (VOX-03..09).
//!
//! Provider adapters turn engine-neutral `cantor_lyrics::Phrase`s into each
//! engine's request format. Network calls, caching and keys come later; the
//! serialisers are pure so they can be contract-tested without a GPU.

pub mod english;
pub mod soulx;

/// Shown wherever "Sing this part" is offered (product decision 2026-11-22:
/// the singing engine, SoulX-Singer, has English but no Swedish phonemes).
/// Swedish UI text lives in the localisation files, not here.
pub const SUNG_LYRICS_NOTICE: &str =
    "Sung lyrics are available in English only. Parts with lyrics in other languages are played by instruments.";

/// Whether sung lyrics can be rendered for a BCP 47 language tag.
pub fn sung_language_supported(lang: &str) -> bool {
    let primary = lang
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    primary == "en"
}
