use vocal_audio::{verify_source, CancellationToken, WavImporter};
use vocal_domain::{
    audio::{ChannelMode, VocalTrack},
    identity::EntityId,
    project::VocalProject,
};
use vocal_project::ProjectStore;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let wav = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/audio/mono-48000.wav")
        });
    let cancel = CancellationToken::default();
    let audio = WavImporter::import_linked(wav, &cancel)?;
    println!(
        "Linked WAV: {} Hz, {} channels, {} frames; SHA-256 {}",
        audio.metadata().sample_rate(),
        audio.metadata().channels(),
        audio.metadata().frames().get(),
        audio.content_hash()
    );
    let track = VocalTrack {
        id: EntityId::new(),
        name: "Vocal".into(),
        source: audio.id(),
        channel_mode: ChannelMode::EqualPowerMono,
    };
    let mut project = VocalProject::new("WAV import example");
    project.attach_audio(audio, track)?;
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("import.vocalproj");
    let store = ProjectStore::default();
    store.save(&path, &project)?;
    let loaded = store.load(&path)?;
    assert_eq!(loaded, project);
    verify_source(&loaded.sources()[0], &cancel)?;
    println!("schema v2: WAV -> source/track -> save -> reopen -> verify OK");
    Ok(())
}
