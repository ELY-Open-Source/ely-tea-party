//! `ely-tea-party hooks install`: Claude Code's hooks report to Ely Tea Party; outside it they do nothing.

use std::path::Path;

use serde_json::{Value, json};

/// The events whose hooks move a character.
const EVENTS: [&str; 9] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PreCompact",
    "SubagentStop",
    "Notification",
    "Stop",
    "SessionEnd",
];

/// The hook's command: a no-op in any terminal but Ely Tea Party.
pub fn command(agent: &str) -> String {
    format!(r#"[ -z "$ELY_HOOK" ] || "$ELY_HOOK" hook {agent} || true"#)
}

/// Codex's `notify` line, which `~/.codex/config.toml` holds one of.
pub fn codex_notify() -> String {
    let script = command("codex").replace("hook codex", r#"hook codex "$1""#);
    format!("notify = [\"sh\", \"-c\", {}, \"sh\"]", json!(script))
}

/// Adds Ely Tea Party's hooks to Claude Code's settings; returns how many it added.
pub fn claude(settings: &mut Value) -> anyhow::Result<usize> {
    let hooks = settings
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("install: Claude's settings are no JSON object"))?
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("install: Claude's `hooks` is no object"))?;
    let command = command("claude");
    let mut added = 0;
    for event in EVENTS {
        let groups = hooks
            .entry(event)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| anyhow::anyhow!("install: Claude's hooks.{event} is no list"))?;
        let present = groups.iter().any(|group| {
            group["hooks"]
                .as_array()
                .is_some_and(|list| list.iter().any(|hook| hook["command"] == command))
        });
        if present {
            continue;
        }
        let hook = json!({ "type": "command", "command": command, "timeout": 5 });
        let group = if event.ends_with("ToolUse") {
            json!({ "matcher": "*", "hooks": [hook] })
        } else {
            json!({ "hooks": [hook] })
        };
        groups.push(group);
        added += 1;
    }
    Ok(added)
}

/// Installs into `~/.claude/settings.json`, kept beside itself as `.bak` first.
pub fn run(home: &Path) -> anyhow::Result<()> {
    let path = home.join(".claude").join("settings.json");
    let mut settings = match std::fs::read_to_string(&path) {
        Ok(text) => {
            let backup = path.with_extension("json.bak");
            std::fs::write(&backup, &text)?;
            println!("Backed up {} to {}", path.display(), backup.display());
            serde_json::from_str(&text)
                .map_err(|error| anyhow::anyhow!("install: {} : {error}", path.display()))?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(error) => anyhow::bail!("install: cannot read {}: {error}", path.display()),
    };
    let added = claude(&mut settings)?;
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(&settings)? + "\n")?;
    println!("Claude Code: {added} hooks added to {}", path.display());
    println!("Codex: put this in ~/.codex/config.toml (it keeps one notify):");
    println!("  {}", codex_notify());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installing_twice_adds_once_and_keeps_the_owners_hooks() {
        let mine = json!({ "type": "command", "command": "say done" });
        let mut settings = json!({ "model": "opus", "hooks": { "Stop": [{ "hooks": [mine] }] } });
        assert_eq!(claude(&mut settings).expect("installed"), EVENTS.len());
        assert_eq!(claude(&mut settings).expect("installed again"), 0);
        assert_eq!(settings["model"], "opus");
        assert_eq!(settings["hooks"]["Stop"].as_array().map(Vec::len), Some(2));
        assert_eq!(settings["hooks"]["PreToolUse"][0]["matcher"], "*");
    }

    #[test]
    fn settings_of_the_wrong_shape_are_refused() {
        assert!(claude(&mut json!([])).is_err());
        assert!(claude(&mut json!({ "hooks": { "Stop": {} } })).is_err());
    }
}
