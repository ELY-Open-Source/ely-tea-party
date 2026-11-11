use super::*;

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("ely-skill-test-{}", uuid::Uuid::new_v4())))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn install_verify_repeat_and_remove_preserves_neighbor_files() {
    let dir = Temp::new();
    install(&dir.0).unwrap();
    let initial = fs::read(dir.0.join(MANIFEST)).unwrap();
    install(&dir.0).unwrap();
    assert_eq!(initial, fs::read(dir.0.join(MANIFEST)).unwrap());
    assert_eq!(status("claude", &dir.0).state, State::Synced);
    fs::write(dir.0.join("notes.md"), "mine").unwrap();
    uninstall(&dir.0).unwrap();
    assert_eq!(fs::read_to_string(dir.0.join("notes.md")).unwrap(), "mine");
    assert_eq!(status("claude", &dir.0).state, State::Missing);
}

#[test]
fn unmanaged_and_user_modified_skills_are_never_overwritten_or_deleted() {
    let dir = Temp::new();
    fs::create_dir_all(&dir.0).unwrap();
    fs::write(dir.0.join("SKILL.md"), "mine").unwrap();
    assert!(install(&dir.0).is_err());
    assert!(uninstall(&dir.0).is_err());
    fs::remove_file(dir.0.join("SKILL.md")).unwrap();
    install(&dir.0).unwrap();
    fs::write(dir.0.join("SKILL.md"), "my edited skill").unwrap();
    assert_eq!(status("codex", &dir.0).state, State::Modified);
    assert!(install(&dir.0).is_err());
    assert!(uninstall(&dir.0).is_err());
    assert_eq!(
        fs::read_to_string(dir.0.join("SKILL.md")).unwrap(),
        "my edited skill"
    );
}

#[test]
fn old_managed_entry_and_interrupted_upgrade_recover() {
    let dir = Temp::new();
    install(&dir.0).unwrap();
    let mut manifest = read_manifest(&dir.0).unwrap().unwrap();
    manifest.version = 0;
    manifest.hash = hash(b"old bootstrap");
    fs::write(dir.0.join("SKILL.md"), "old bootstrap").unwrap();
    fs::write(dir.0.join(MANIFEST), serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert_eq!(status("claude", &dir.0).state, State::UpdateAvailable);
    install(&dir.0).unwrap();
    manifest.version = SKILL_VERSION;
    manifest.pending = Some(hash(SKILL.as_bytes()));
    fs::write(dir.0.join(MANIFEST), serde_json::to_vec(&manifest).unwrap()).unwrap();
    install(&dir.0).unwrap();
    assert_eq!(status("claude", &dir.0).state, State::Synced);
    fs::remove_file(dir.0.join("SKILL.md")).unwrap();
    install(&dir.0).unwrap();
    assert_eq!(status("claude", &dir.0).state, State::Synced);
}

#[test]
fn newer_installer_manifest_is_not_downgraded() {
    let dir = Temp::new();
    install(&dir.0).unwrap();
    let mut manifest = read_manifest(&dir.0).unwrap().unwrap();
    manifest.version = SKILL_VERSION + 1;
    fs::write(dir.0.join(MANIFEST), serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(install(&dir.0).is_err());
    assert_eq!(status("claude", &dir.0).state, State::Failed);
}

#[test]
fn startup_sync_updates_only_previously_managed_targets() {
    let home = Temp::new();
    let targets = targets(&home.0);
    assert!(
        refresh(&home.0)
            .iter()
            .all(|status| status.state == State::Missing)
    );
    assert!(!home.0.exists());
    install(&targets[0].1).unwrap();
    fs::remove_file(targets[0].1.join("SKILL.md")).unwrap();
    let statuses = refresh(&home.0);
    assert_eq!(statuses[0].state, State::Synced);
    assert_eq!(statuses[1].state, State::Missing);
}

#[cfg(unix)]
#[test]
fn symlinked_entry_is_not_followed() {
    let dir = Temp::new();
    let other = Temp::new();
    fs::create_dir_all(&dir.0).unwrap();
    fs::create_dir_all(&other.0).unwrap();
    fs::write(other.0.join("file"), "mine").unwrap();
    std::os::unix::fs::symlink(other.0.join("file"), dir.0.join("SKILL.md")).unwrap();
    assert!(install(&dir.0).is_err());
    assert_eq!(fs::read_to_string(other.0.join("file")).unwrap(), "mine");
}
