use std::{
    path::PathBuf,
    process::{Command, Output},
};

struct Home(PathBuf);
impl Home {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("ely-cli-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_ely-tea-party"))
            .args(args)
            .env_clear()
            .env("HOME", &self.0)
            .env("USERPROFILE", &self.0)
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
fn cli_version_and_help_do_not_start_a_gui_or_create_state() {
    let home = Home::new();
    let version = home.run(&["--version"]);
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains(env!("CARGO_PKG_VERSION")));
    let help = home.run(&["msg", "--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("msg guide"));
    assert_eq!(std::fs::read_dir(&home.0).unwrap().count(), 0);
}

#[test]
fn missing_connection_and_unknown_commands_fail_without_starting_app() {
    let home = Home::new();
    for args in [
        &["msg", "list"][..],
        &["msg", "guide"],
        &["msg", "unsupported"],
    ] {
        let result = home.run(args);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        let error = String::from_utf8_lossy(&result.stderr);
        assert!(error.contains("messages:"), "{error}");
    }
    assert_eq!(std::fs::read_dir(&home.0).unwrap().count(), 0);
}

#[test]
fn installed_binary_manages_both_skill_entries_and_protects_edits() {
    let home = Home::new();
    for agent in ["claude", "codex"] {
        let result = home.run(&["skills", "install", agent]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(value["state"], "synced");
    }
    let path = home.0.join(".agents/skills/ely-session-messaging/SKILL.md");
    let bootstrap = std::fs::read_to_string(&path).unwrap();
    assert!(bootstrap.contains("\"$ELY_CLI\" msg guide"));
    assert!(!bootstrap.contains("/Applications"));
    std::fs::write(&path, "user customization").unwrap();
    assert!(!home.run(&["skills", "sync"]).status.success());
    assert!(!home.run(&["skills", "uninstall", "codex"]).status.success());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "user customization"
    );
    let statuses = home.run(&["skills", "status"]);
    let statuses: serde_json::Value = serde_json::from_slice(&statuses.stdout).unwrap();
    assert_eq!(statuses[0]["state"], "synced");
    assert_eq!(statuses[1]["state"], "modified");
    assert!(
        home.run(&["skills", "uninstall", "claude"])
            .status
            .success()
    );
    assert!(
        !home
            .0
            .join(".claude/skills/ely-session-messaging/SKILL.md")
            .exists()
    );
}
