use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::Write,
};
use vocal_domain::{identity::EntityId, project::VocalProject};
use vocal_project::{
    MigrationRegistry, ProjectError, ProjectMigration, ProjectStore, MAX_JSON_BYTES,
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

const ID: &str = "cdb6a439-68bc-4c50-8689-a5adb2689c00";

#[test]
fn real_v1_migration_preserves_source_bytes_and_saves_v2() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v1.vocalproj");
    package(
        &path,
        manifest(1),
        include_bytes!("fixtures/v1-project.json"),
        None,
    );
    let before = fs::read(&path).unwrap();
    let store = ProjectStore::default();
    let project = store.load(&path).unwrap();
    assert_eq!(project.id().to_string(), ID);
    assert_eq!(project.name(), "V1 compatibility fixture");
    assert!(project.sources().is_empty() && project.tracks().is_empty());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(matches!(
        ProjectStore::new(MigrationRegistry::empty()).load(&path),
        Err(ProjectError::Migration(_))
    ));
    store.save(&path, &project).unwrap();
    let mut zip = ZipArchive::new(File::open(&path).unwrap()).unwrap();
    let saved: Value = serde_json::from_reader(zip.by_name("manifest.json").unwrap()).unwrap();
    assert_eq!(saved["schemaVersion"], 2);
    assert_eq!(store.load(&path).unwrap(), project);
}

#[test]
fn audio_roundtrip_does_not_require_online_source_or_modify_audio() {
    use vocal_audio::{verify_source, AudioError, CancellationToken, WavImporter};
    use vocal_domain::audio::{ChannelMode, VocalTrack};
    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("vocal.wav");
    let fixture = include_bytes!("../../../fixtures/audio/stereo-48000.wav");
    fs::write(&wav, fixture).unwrap();
    let source = WavImporter::import_linked(&wav, &CancellationToken::default()).unwrap();
    let track = VocalTrack {
        id: EntityId::new(),
        name: "Voice".into(),
        source: source.id(),
        channel_mode: ChannelMode::EqualPowerMono,
    };
    let mut project = VocalProject::new("Linked project");
    project.attach_audio(source, track).unwrap();
    let path = dir.path().join("audio.vocalproj");
    let store = ProjectStore::default();
    store.save(&path, &project).unwrap();
    assert_eq!(fs::read(&wav).unwrap(), fixture);
    fs::remove_file(wav).unwrap();
    let loaded = store.load(&path).unwrap();
    assert_eq!(loaded, project);
    assert!(matches!(
        verify_source(&loaded.sources()[0], &CancellationToken::default()),
        Err(AudioError::Io(_))
    ));
    store.save(&path, &loaded).unwrap();
    assert_eq!(store.load(&path).unwrap(), project);
}

#[test]
fn v2_rejects_bad_references_metadata_hash_and_unknown_observations() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad-v2.vocalproj");
    let source_id = EntityId::new().to_string();
    let track_id = EntityId::new().to_string();
    let base = json!({"id":ID,"name":"Audio", "sources":[{
        "id":source_id, "uri":{"kind":"linked","absoluteFallback":"C:/offline/vocal.wav","relativePath":null},
        "contentHash":"ab".repeat(32),"sizeBytes":1000,"sampleRate":48000,"channels":1,"frames":480
    }],"tracks":[{"id":track_id,"name":"Voice","source":source_id,"channelMode":"equalPowerMono"}]});
    package(
        &path,
        manifest(2),
        &serde_json::to_vec(&base).unwrap(),
        None,
    );
    assert!(ProjectStore::default().load(&path).is_ok());
    for variant in 0..7 {
        let mut bad = base.clone();
        match variant {
            0 => bad["tracks"][0]["source"] = json!(EntityId::new().to_string()),
            1 => bad["sources"][0]["sampleRate"] = json!(0),
            2 => bad["sources"][0]["contentHash"] = json!("bad hash"),
            3 => bad["tracks"][0]["id"] = json!(ID),
            4 => bad["sources"][0]["provenance"] = json!({"retain":true}),
            5 => bad["userOverride"] = json!({"retain":true}),
            _ => bad["tracks"][0]["channelMode"] = json!("unknown"),
        }
        package(&path, manifest(2), &serde_json::to_vec(&bad).unwrap(), None);
        let before = fs::read(&path).unwrap();
        assert!(
            ProjectStore::default().load(&path).is_err(),
            "variant {variant}"
        );
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}

#[test]
fn frozen_v1_fixture_remains_readable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v1.vocalproj");
    package(
        &path,
        serde_json::from_str(include_str!("fixtures/v1-manifest.json")).unwrap(),
        include_bytes!("fixtures/v1-project.json"),
        None,
    );
    let loaded = ProjectStore::default().load(&path).unwrap();
    assert_eq!(loaded.id().to_string(), ID);
    assert_eq!(loaded.name(), "V1 compatibility fixture");
}

fn manifest(version: u32) -> Value {
    json!({"format":"vocal-project", "schemaVersion":version, "createdWith":"fixture", "projectId":ID})
}

fn package(path: &std::path::Path, manifest: Value, payload: &[u8], extra: Option<&str>) {
    let mut zip = ZipWriter::new(File::create(path).unwrap());
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    zip.start_file("manifest.json", options).unwrap();
    zip.write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    zip.start_file("project.json", options).unwrap();
    zip.write_all(payload).unwrap();
    if let Some(name) = extra {
        zip.start_file(name, options).unwrap();
        zip.write_all(b"preserve me").unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn create_rename_save_reopen_retains_identity_and_unicode() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("demo.vocalproj");
    let mut project = VocalProject::new("Demo");
    let id = project.id();
    assert_ne!(id, VocalProject::new("Demo").id());
    project.rename("未命名 🎵\nVocal");
    let store = ProjectStore::default();
    store.save(&path, &project).unwrap();
    let mut reopened = store.load(&path).unwrap();
    assert_eq!(reopened, project);
    assert_eq!(reopened.id(), id);
    reopened.rename("Second save");
    store.save(&path, &reopened).unwrap();
    assert_eq!(store.load(&path).unwrap(), reopened);
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn saved_container_has_explicit_current_manifest_and_separate_payload() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("format.vocalproj");
    let project = VocalProject::new("");
    ProjectStore::default().save(&path, &project).unwrap();
    let mut zip = ZipArchive::new(File::open(path).unwrap()).unwrap();
    assert_eq!(zip.len(), 2);
    let manifest: Value = serde_json::from_reader(zip.by_name("manifest.json").unwrap()).unwrap();
    assert_eq!(manifest["schemaVersion"], 2);
    assert_eq!(manifest["format"], "vocal-project");
    assert_eq!(manifest["projectId"], project.id().to_string());
    let payload: Value = serde_json::from_reader(zip.by_name("project.json").unwrap()).unwrap();
    assert_eq!(
        payload,
        json!({"id": project.id().to_string(), "name":"", "sources":[], "tracks":[]})
    );
}

#[test]
fn invalid_schema_and_manifest_are_rejected_without_modifying_source() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.vocalproj");
    for bad in [
        json!({"format":"wrong", "schemaVersion":1, "createdWith":"fixture", "projectId":ID}),
        json!({"format":"vocal-project", "createdWith":"fixture", "projectId":ID}),
        json!({"format":"vocal-project", "schemaVersion":1, "createdWith":"fixture", "projectId":"bad"}),
        json!({"format":"vocal-project", "schemaVersion":1, "createdWith":"fixture", "projectId":ID, "unknown":true}),
    ] {
        package(
            &path,
            bad,
            &serde_json::to_vec(&json!({"id":ID,"name":"Demo"})).unwrap(),
            None,
        );
        let before = fs::read(&path).unwrap();
        assert!(ProjectStore::default().load(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
    }
    package(&path, manifest(3), b"invalid even as JSON", None);
    assert!(matches!(
        ProjectStore::default().load(&path),
        Err(ProjectError::UnsupportedVersion(3))
    ));
    package(&path, manifest(0), b"{}", None);
    assert!(matches!(
        ProjectStore::default().load(&path),
        Err(ProjectError::Migration(_))
    ));
}

#[test]
fn unknown_revision_provenance_and_duplicate_fields_are_not_discarded() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("richer.vocalproj");
    for field in [
        "userOverride",
        "provenance",
        "rawObservations",
        "tracks",
        "tempoMap",
    ] {
        let payload = json!({"id":ID, "name":"Demo", field:{"keep":"original"}});
        package(
            &path,
            manifest(1),
            &serde_json::to_vec(&payload).unwrap(),
            None,
        );
        assert!(ProjectStore::default().load(&path).is_err(), "{field}");
    }
    let duplicate = format!(r#"{{"id":"{ID}","name":"first","name":"second"}}"#);
    package(&path, manifest(1), duplicate.as_bytes(), None);
    assert!(ProjectStore::default().load(&path).is_err());
}

#[test]
fn mismatched_or_invalid_payload_identity_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad-id.vocalproj");
    for id in ["invalid".into(), EntityId::new().to_string()] {
        package(
            &path,
            manifest(1),
            &serde_json::to_vec(&json!({"id":id,"name":"Demo"})).unwrap(),
            None,
        );
        assert!(matches!(
            ProjectStore::default().load(&path),
            Err(ProjectError::Invalid(_))
        ));
    }
}

#[test]
fn corrupt_missing_extra_and_oversized_content_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.vocalproj");
    fs::write(&path, b"not a zip").unwrap();
    assert!(ProjectStore::default().load(&path).is_err());
    ZipWriter::new(File::create(&path).unwrap())
        .finish()
        .unwrap();
    assert!(ProjectStore::default().load(&path).is_err());
    for extra in ["analysis/observations/raw.json", "../escape"] {
        package(&path, manifest(1), b"{}", Some(extra));
        assert!(ProjectStore::default().load(&path).is_err());
    }
    package(
        &path,
        manifest(1),
        &vec![b' '; MAX_JSON_BYTES as usize + 1],
        None,
    );
    assert!(ProjectStore::default().load(&path).is_err());
    File::create(&path)
        .unwrap()
        .set_len(MAX_JSON_BYTES * 3)
        .unwrap();
    assert!(ProjectStore::default().load(&path).is_err());
}

#[test]
fn failed_save_leaves_previous_project_and_no_temp_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("precious.vocalproj");
    let store = ProjectStore::default();
    let original = VocalProject::new("keep me");
    store.save(&path, &original).unwrap();
    let before = fs::read(&path).unwrap();
    let mut too_large = original.clone();
    too_large.rename("x".repeat(MAX_JSON_BYTES as usize));
    assert!(store.save(&path, &too_large).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(store.load(&path).unwrap(), original);
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    // Force a failure during final replacement, after writing and validating temp.
    let blocked = dir.path().join("directory.vocalproj");
    fs::create_dir(&blocked).unwrap();
    fs::write(blocked.join("sentinel"), b"unchanged").unwrap();
    assert!(store.save(&blocked, &original).is_err());
    assert_eq!(fs::read(blocked.join("sentinel")).unwrap(), b"unchanged");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
}

// Synthetic legacy fixture only: schema 0 was never a released project format.
struct TestMigration {
    from: u32,
    to: u32,
    fail: bool,
}
impl ProjectMigration for TestMigration {
    fn from_version(&self) -> u32 {
        self.from
    }
    fn to_version(&self) -> u32 {
        self.to
    }
    fn migrate(&self, mut raw: Value) -> Result<Value, ProjectError> {
        if self.fail {
            return Err(ProjectError::Migration("fixture failure".into()));
        }
        let title = raw.as_object_mut().unwrap().remove("title").unwrap();
        raw["name"] = title;
        Ok(raw)
    }
}

#[test]
fn migration_registration_rejects_skips_backwards_duplicates_and_future_steps() {
    let mut registry = MigrationRegistry::default();
    for (from, to) in [(0, 0), (1, 0), (0, 2), (1, 2), (u32::MAX, 0)] {
        assert!(registry
            .register(Box::new(TestMigration {
                from,
                to,
                fail: false
            }))
            .is_err());
    }
    registry
        .register(Box::new(TestMigration {
            from: 0,
            to: 1,
            fail: false,
        }))
        .unwrap();
    assert!(registry
        .register(Box::new(TestMigration {
            from: 0,
            to: 1,
            fail: false
        }))
        .is_err());
}

#[test]
fn migration_is_in_memory_then_explicit_save_writes_current_schema() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("synthetic-legacy.vocalproj");
    let output = dir.path().join("upgraded.vocalproj");
    package(
        &source,
        manifest(0),
        &serde_json::to_vec(&json!({"id":ID,"title":"Old name"})).unwrap(),
        None,
    );
    let before = fs::read(&source).unwrap();
    let mut registry = MigrationRegistry::default();
    registry
        .register(Box::new(TestMigration {
            from: 0,
            to: 1,
            fail: false,
        }))
        .unwrap();
    let store = ProjectStore::new(registry);
    let project = store.load(&source).unwrap();
    assert_eq!(project.id().to_string(), ID);
    assert_eq!(project.name(), "Old name");
    assert_eq!(fs::read(&source).unwrap(), before);
    store.save(&output, &project).unwrap();
    assert_eq!(ProjectStore::default().load(&output).unwrap(), project);
}

#[test]
fn migration_failure_and_unrecognized_output_do_not_change_source() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy.vocalproj");
    package(
        &path,
        manifest(0),
        &serde_json::to_vec(&json!({"id":ID,"title":"Old","provenance":{"keep":true}})).unwrap(),
        None,
    );
    let before = fs::read(&path).unwrap();
    for fail in [true, false] {
        let mut registry = MigrationRegistry::default();
        registry
            .register(Box::new(TestMigration {
                from: 0,
                to: 1,
                fail,
            }))
            .unwrap();
        assert!(ProjectStore::new(registry).load(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}
