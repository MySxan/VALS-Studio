use vocal_domain::project::VocalProject;
use vocal_project::ProjectStore;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("demo.vocalproj");
    let store = ProjectStore::default();
    let mut project = VocalProject::new("手工项目");
    store.save(&path, &project)?;
    assert_eq!(store.load(&path)?, project);
    project.rename("重命名后仍保持身份");
    store.save(&path, &project)?;
    let reopened = store.load(&path)?;
    assert_eq!(reopened, project);
    println!("schema v2 ZIP: create -> save -> reopen -> rename -> save -> reopen OK");
    println!("{}: {}", reopened.id(), reopened.name());
    Ok(())
}
