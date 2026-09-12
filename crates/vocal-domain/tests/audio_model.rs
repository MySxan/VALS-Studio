use vocal_domain::{audio::*, identity::EntityId, project::VocalProject, time::Samples};

fn source() -> AudioSource {
    AudioSource::new(
        EntityId::new(),
        AudioUri::Linked {
            absolute_fallback: "C:/foreign/vocal.wav".into(),
            relative_path: Some("source/vocal.wav".into()),
        },
        ContentHash([42; 32]),
        1000,
        AudioMetadata::new(48000, 1, Samples::new(480)).unwrap(),
    )
    .unwrap()
}

#[test]
fn attachment_is_atomic_and_references_are_validated() {
    let mut project = VocalProject::new("Demo");
    let audio = source();
    let track = VocalTrack {
        id: EntityId::new(),
        name: "Vocal".into(),
        source: audio.id(),
        channel_mode: ChannelMode::EqualPowerMono,
    };
    let mut invalid = track.clone();
    invalid.source = EntityId::new();
    let before = project.clone();
    assert!(project.attach_audio(audio.clone(), invalid).is_err());
    assert_eq!(project, before);
    project.attach_audio(audio.clone(), track.clone()).unwrap();
    let before = project.clone();
    assert!(project.attach_audio(audio, track).is_err());
    assert_eq!(project, before);
    let mut tracks = project.tracks().to_vec();
    tracks[0].source = EntityId::new();
    assert!(VocalProject::with_audio(
        project.id(),
        "Demo".into(),
        project.sources().to_vec(),
        tracks
    )
    .is_err());
    let mut tracks = project.tracks().to_vec();
    tracks[0].id = project.id();
    assert!(VocalProject::with_audio(
        project.id(),
        "Demo".into(),
        project.sources().to_vec(),
        tracks
    )
    .is_err());
}

#[test]
fn invalid_metadata_hash_and_relative_paths_are_rejected() {
    for (rate, channels, frames) in [(0, 1, 480), (48000, 0, 480), (48000, 1, 0), (48000, 1, -1)] {
        assert!(AudioMetadata::new(rate, channels, Samples::new(frames)).is_err());
    }
    for hash in ["", "not a hash", &"z".repeat(64)] {
        assert!(hash.parse::<ContentHash>().is_err());
    }
    let hash = ContentHash([42; 32]);
    assert_eq!(hash.to_string().parse::<ContentHash>().unwrap(), hash);
    for path in [
        "../vocal.wav",
        "..\\vocal.wav",
        "/vocal.wav",
        "C:/vocal.wav",
    ] {
        assert!(AudioSource::new(
            EntityId::new(),
            AudioUri::Linked {
                absolute_fallback: "/vocal.wav".into(),
                relative_path: Some(path.into())
            },
            hash,
            1000,
            source().metadata()
        )
        .is_err());
    }
}

#[test]
fn source_location_update_is_atomic_and_cannot_change_content_facts() {
    let mut project = VocalProject::new("Location");
    let audio = source();
    project
        .attach_audio(
            audio.clone(),
            VocalTrack {
                id: EntityId::new(),
                name: "Vocal".into(),
                source: audio.id(),
                channel_mode: ChannelMode::Left,
            },
        )
        .unwrap();
    let before = project.clone();
    let invalid = AudioUri::Linked {
        absolute_fallback: "".into(),
        relative_path: None,
    };
    assert!(project.set_source_uri(audio.id(), invalid).is_err());
    assert!(project
        .set_source_uri(EntityId::new(), audio.uri().clone())
        .is_err());
    assert_eq!(project, before);
    let uri = AudioUri::Linked {
        absolute_fallback: "/moved/vocal.wav".into(),
        relative_path: None,
    };
    project.set_source_uri(audio.id(), uri.clone()).unwrap();
    assert_eq!(project.id(), before.id());
    assert_eq!(project.name(), before.name());
    assert_eq!(project.tracks(), before.tracks());
    assert_eq!(
        project.sources(),
        &[AudioSource::new(
            audio.id(),
            uri,
            audio.content_hash(),
            audio.size_bytes(),
            audio.metadata()
        )
        .unwrap()]
    );
}
