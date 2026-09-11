use std::{fs, path::PathBuf};
use vocal_audio::{verify_source, AudioError, CancellationToken, WavImporter};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/audio")
        .join(name)
}

#[test]
fn mono_and_stereo_keep_original_metadata_bytes_and_stable_content_hash() {
    for (name, channels, frames, hash) in [
        (
            "mono-48000.wav",
            1,
            480,
            "0ff52963ef493d16ca18d47d1bd782fb7d06801f8f4d2e3847cbea3680bc86bd",
        ),
        (
            "stereo-48000.wav",
            2,
            960,
            "3bf26cbf3c9f632a2935804ac55f9fd3484ae3e5fc10fadb503ec86d8a0a41b5",
        ),
    ] {
        let path = fixture(name);
        let before = fs::read(&path).unwrap();
        let cancel = CancellationToken::default();
        let source = WavImporter::import_linked(&path, &cancel).unwrap();
        assert_eq!(source.metadata().sample_rate(), 48000);
        assert_eq!(source.metadata().channels(), channels);
        assert_eq!(source.metadata().frames().get(), frames);
        assert_eq!(source.metadata().duration().get(), frames as f64 / 48000.0);
        assert_eq!(source.size_bytes(), before.len() as u64);
        assert_eq!(source.content_hash().to_string(), hash);
        verify_source(&source, &cancel).unwrap();
        assert_eq!(fs::read(path).unwrap(), before);
    }
}

#[test]
fn identity_is_distinct_from_content_and_filename() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("renamed.bin");
    fs::copy(fixture("mono-48000.wav"), &path).unwrap();
    let first =
        WavImporter::import_linked(fixture("mono-48000.wav"), &CancellationToken::default())
            .unwrap();
    let second = WavImporter::import_linked(path, &CancellationToken::default()).unwrap();
    assert_ne!(first.id(), second.id());
    assert_eq!(first.content_hash(), second.content_hash());
}

#[test]
fn cancellation_prevents_import_and_verification() {
    let cancel = CancellationToken::default();
    let source = WavImporter::import_linked(fixture("mono-48000.wav"), &cancel).unwrap();
    cancel.clone().cancel();
    assert!(matches!(
        WavImporter::import_linked("missing.wav", &cancel),
        Err(AudioError::Cancelled)
    ));
    assert!(matches!(
        verify_source(&source, &cancel),
        Err(AudioError::Cancelled)
    ));
}

#[test]
fn changed_and_missing_source_are_explicit_errors() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("linked.wav");
    fs::copy(fixture("mono-48000.wav"), &path).unwrap();
    let source = WavImporter::import_linked(&path, &CancellationToken::default()).unwrap();
    let mut bytes = fs::read(&path).unwrap();
    bytes[44] ^= 1; // Same size and metadata, different audio content.
    fs::write(&path, bytes).unwrap();
    assert!(matches!(
        verify_source(&source, &CancellationToken::default()),
        Err(AudioError::SourceChanged)
    ));
    fs::remove_file(path).unwrap();
    assert!(matches!(
        verify_source(&source, &CancellationToken::default()),
        Err(AudioError::Io(_))
    ));
}

#[test]
fn rejects_nonwav_truncated_and_empty_audio() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("invalid.wav");
    let original = fs::read(fixture("mono-48000.wav")).unwrap();
    let mut empty = original[..44].to_vec();
    empty[4..8].copy_from_slice(&36_u32.to_le_bytes());
    empty[40..44].copy_from_slice(&0_u32.to_le_bytes());
    for bytes in [
        b"not a WAV despite its extension".to_vec(),
        original[..30].to_vec(),
        empty,
    ] {
        fs::write(&path, &bytes).unwrap();
        assert!(WavImporter::import_linked(&path, &CancellationToken::default()).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}
