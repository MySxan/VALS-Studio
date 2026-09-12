//! Linked file registration and verification. No inference or decoded PCM artifacts.
mod decode;
mod import;
mod io;
pub use decode::{decode_verified_pcm, DecodeAnalyzer};
pub use import::WavImporter;
pub use io::{verify_source, AudioError};
pub use vocal_analysis_api::CancellationToken;
