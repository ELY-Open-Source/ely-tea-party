//! Agents whose JSON config holds Claude Code's shape of hooks: `{"hooks": {EVENT: [{"hooks": [COMMAND]}]}}`.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use super::{Installed, command, command_for, files, folder};

/// Claude Code's events whose hooks move a character.
const CLAUDE: [&str; 13] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "PreCompact",
    "SubagentStart",
    "SubagentStop",
    "PermissionRequest",
    "Notification",
    "Stop",
    "StopFailure",
    "SessionEnd",
];

const GEMINI: [&str; 8] = [
    "SessionStart",
    "BeforeAgent",
    "AfterAgent",
    "BeforeTool",
    "AfterTool",
    "Notification",
    "PreCompress",
    "SessionEnd",
];

const GROK: [&str; 14] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "PermissionDenied",
    "Stop",
    "StopFailure",
    "Notification",
    "SubagentStart",
    "SubagentStop",
    "PreCompact",
    "PostCompact",
    "SessionEnd",
];

/// The events dscode's hooks file is known to take.
const DSCODE: [&str; 5] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "Stop",
];

/// Antigravity's events; its `SessionEnd` comes after each turn. No `PreToolUse`: Antigravity denies a tool whose pre-tool hook names no decision.
const ANTIGRAVITY: [&str; 7] = [
    "SessionStart",
    "PreInvocation",
    "PostToolUse",
    "Stop",
    "turn-completion",
    "Notification",
    "SessionEnd",
];

/// A tool's events, whose groups name the tools they match.
fn on_tools(event: &str) -> bool {
    event.contains("ToolUse") || event == "PermissionRequest"
}

/// A hook group running `command`, `timeout` long in the agent's unit.
pub(super) fn group(command: String, timeout: u64, matcher: bool) -> Value {
    let hook = json!({ "type": "command", "command": command, "timeout": timeout });
    if matcher {
        json!({ "matcher": "*", "hooks": [hook] })
    } else {
        json!({ "hooks": [hook] })
    }
}

/// Whether one of `groups` runs `command`.
fn runs(groups: &[Value], command: &str) -> bool {
    groups.iter().any(|group| {
        group["command"] == command
            || group["hooks"]
                .as_array()
                .is_some_and(|hooks| hooks.iter().any(|hook| hook["command"] == command))
    })
}

/// Adds `group(event)` under each of `events` in `hooks` whose groups lack its command; returns how many.
pub(super) fn add(
    hooks: &mut Map<String, Value>,
    events: &[&str],
    command: impl Fn(&str) -> String,
    group: impl Fn(&str, String) -> Value,
) -> anyhow::Result<usize> {
    let mut added = 0;
    for event in events {
        let groups = hooks
            .entry(*event)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| anyhow::anyhow!("its hooks.{event} is no list"))?;
        let command = command(event);
        if !runs(groups, &command) {
            groups.push(group(event, command));
            added += 1;
        }
    }
    Ok(added)
}

/// `settings`' `hooks` object, made when missing.
pub(super) fn hooks(settings: &mut Value) -> anyhow::Result<&mut Map<String, Value>> {
    settings
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("its settings are no JSON object"))?
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("its `hooks` is no object"))
}

/// Adds Ely Tea Party's hooks to Claude Code's settings; returns how many it added.
pub fn claude(settings: &mut Value) -> anyhow::Result<usize> {
    let command = command("claude");
    add(
        hooks(settings)?,
        &CLAUDE,
        |_| command.clone(),
        |event, command| group(command, 5, on_tools(event)),
    )
}

/// Applies `edit` to the JSON file at `path`, which holds an object, and writes it if it changed.
fn edit_json(
    path: PathBuf,
    edit: impl FnOnce(&mut Value) -> anyhow::Result<usize>,
) -> anyhow::Result<Installed> {
    let (old, mut value) = files::read_json(&path)?;
    let added = edit(&mut value)?;
    files::write_json(&path, old.as_deref(), &value)?;
    Ok(Installed { path, added })
}

pub fn claude_file(home: &Path) -> anyhow::Result<Installed> {
    let config = folder("CLAUDE_CONFIG_DIR", home.join(".claude"));
    edit_json(config.join("settings.json"), |settings| {
        let speed = super::statusline::install(settings)?;
        Ok(claude(settings)? + speed)
    })
}

pub fn gemini(home: &Path) -> anyhow::Result<Installed> {
    edit_json(home.join(".gemini").join("settings.json"), |settings| {
        // Gemini counts timeouts in milliseconds.
        let each = |event: &str| command_for("gemini", event);
        add(hooks(settings)?, &GEMINI, each, |_, command| {
            group(command, 5000, false)
        })
    })
}

pub fn dscode(home: &Path) -> anyhow::Result<Installed> {
    let state = folder("DSCODE_HOME", home.join(".local/share/dscode-state"));
    edit_json(state.join("config/hooks.local.json"), |settings| {
        let each = |event: &str| command_for("dscode", event);
        add(hooks(settings)?, &DSCODE, each, |_, command| {
            group(command, 5, false)
        })
    })
}

pub fn grok(home: &Path) -> anyhow::Result<Installed> {
    let grok = std::env::var_os("GROK_HOME")
        .filter(|value| !value.is_empty())
        .map_or(home.join(".grok"), PathBuf::from);
    grok_in(&grok.join("hooks"))
}

/// Grok reads every file in its hooks folder, so it gets one of its own.
fn grok_in(hooks: &Path) -> anyhow::Result<Installed> {
    let path = hooks.join(format!("{}.json", super::MARK));
    let groups: Map<String, Value> = GROK
        .iter()
        .map(|event| {
            let hook = group(command_for("grok", event), 5, on_tools(event));
            (event.to_string(), json!([hook]))
        })
        .collect();
    let value = json!({ "hooks": groups });
    let old = files::read(&path)?;
    let wrote = files::write_json(&path, old.as_deref(), &value)?;
    Ok(Installed {
        path,
        added: if wrote { GROK.len() } else { 0 },
    })
}

/// Antigravity names each tool's hook group; Ely Tea Party's is its own, written whole.
pub fn antigravity(home: &Path) -> anyhow::Result<Installed> {
    edit_json(home.join(".gemini/config/hooks.json"), |config| {
        let ours: Map<String, Value> = ANTIGRAVITY
            .iter()
            .map(|event| {
                let hook = json!({ "type": "command", "command": command_for("agy", event), "timeout": 10 });
                let entry = if on_tools(event) {
                    json!({ "matcher": "*", "hooks": [hook] })
                } else {
                    hook
                };
                (event.to_string(), json!([entry]))
            })
            .collect();
        let config = config
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("its hooks file is no JSON object"))?;
        let ours = Value::Object(ours);
        let changed = config.get(super::MARK) != Some(&ours);
        config.insert(super::MARK.into(), ours);
        Ok(if changed { ANTIGRAVITY.len() } else { 0 })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installing_twice_adds_once_and_keeps_the_owners_hooks() {
        let mine = json!({ "type": "command", "command": "say done" });
        let mut settings = json!({ "model": "opus", "hooks": { "Stop": [{ "hooks": [mine] }] } });
        assert_eq!(claude(&mut settings).expect("installed"), CLAUDE.len());
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

    #[test]
    fn each_agent_gets_its_events_once_in_its_own_files() {
        let home = std::env::temp_dir().join(format!("ely-groups-{}", std::process::id()));
        let gemini_file = home.join(".gemini/settings.json");
        std::fs::create_dir_all(gemini_file.parent().expect("a folder")).expect("made");
        std::fs::write(
            &gemini_file,
            r#"{"theme":"x","hooks":{"AfterTool":[{"hooks":[]}]}}"#,
        )
        .expect("written");
        assert_eq!(gemini(&home).expect("installed").added, GEMINI.len());
        assert_eq!(gemini(&home).expect("again").added, 0);
        let (_, value) = files::read_json(&gemini_file).expect("read");
        assert_eq!(value["theme"], "x");
        assert_eq!(
            value["hooks"]["AfterTool"].as_array().map(Vec::len),
            Some(2)
        );
        assert_eq!(value["hooks"]["BeforeTool"][0]["hooks"][0]["timeout"], 5000);
        let backup = home.join(".gemini/settings.json.ely-tea-party.bak");
        assert!(backup.exists(), "the first file kept");
        assert_eq!(
            antigravity(&home).expect("installed").added,
            ANTIGRAVITY.len()
        );
        assert_eq!(antigravity(&home).expect("again").added, 0);
        let hooks = home.join(".grok/hooks");
        assert_eq!(grok_in(&hooks).expect("installed").added, GROK.len());
        assert_eq!(grok_in(&hooks).expect("again").added, 0);
        std::fs::remove_dir_all(&home).expect("cleaned");
    }
}
