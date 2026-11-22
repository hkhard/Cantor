//! Vocal rendering providers (VOX-03..09).
//!
//! Provider adapters turn engine-neutral `cantor_lyrics::Phrase`s into each
//! engine's request format. Network calls, caching and keys come later; the
//! serialisers are pure so they can be contract-tested without a GPU.

pub mod soulx;
