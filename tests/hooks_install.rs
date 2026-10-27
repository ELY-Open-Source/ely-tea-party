#![cfg(unix)]

use std::{os::unix::fs::PermissionsExt, path::PathBuf, process::Command};

struct Home(PathBuf);

impl Home {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("ely-install-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        Self(dir)
    }

    fn agent(&self, name: &str) {
        let path = self.0.join("bin").join(name);
        std::fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
    }

    fn install(&self) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_ely-tea-party"))
            .args(["hooks", "install"])
            .env_clear()
            .env("HOME", &self.0)
            .env("PATH", self.0.join("bin"))
            .output()
            .unwrap()
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn no_detected_agents_is_not_reported_as_success() {
    let home = Home::new();
    let output = home.install();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("no supported agents found"));
    assert!(!home.0.join(".claude").exists());
}

#[test]
fn installing_backs_up_configuration_and_a_repeat_keeps_it() {
    let home = Home::new();
    home.agent("claude");
    let dir = home.0.join(".claude");
    std::fs::create_dir(&dir).unwrap();
    let original = "{\"model\":\"owner-choice\"}\n";
    let path = dir.join("settings.json");
    let backup = dir.join("settings.json.ely-tea-party.bak");
    std::fs::write(&path, original).unwrap();
    let first = home.install();
    assert!(first.status.success(), "{first:?}");
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), original);
    let installed = std::fs::read_to_string(&path).unwrap();
    assert!(installed.contains("owner-choice"));
    let second = home.install();
    assert!(second.status.success(), "{second:?}");
    assert!(String::from_utf8_lossy(&second.stdout).contains("up to date"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), installed);
    assert_eq!(std::fs::read_to_string(backup).unwrap(), original);
}

#[test]
fn one_agent_failure_keeps_other_successes_and_returns_failure() {
    let home = Home::new();
    home.agent("claude");
    home.agent("codex");
    let dir = home.0.join(".claude");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("settings.json"), "broken JSON").unwrap();
    let output = home.install();
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("claude    failed:"), "{stdout}");
    assert!(stdout.contains("codex     installed:"), "{stdout}");
    assert!(home.0.join(".codex/hooks.json").exists());
    assert_eq!(
        std::fs::read_to_string(dir.join("settings.json")).unwrap(),
        "broken JSON"
    );
}
