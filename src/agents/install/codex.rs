//! Codex's hooks: hooks.json in Claude Code's shape, turned on and trusted in config.toml.

use std::path::Path;

use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{Installed, MARK, command_for, files, folder, groups};

const EVENTS: [&str; 11] = [
    "SessionStart",
    "UserPromptSubmit",
    "Stop",
    "PermissionRequest",
    "PreToolUse",
    "PostToolUse",
    "PreCompact",
    "PostCompact",
    "SubagentStart",
    "SubagentStop",
    "SessionEnd",
];

/// Codex gives `SessionEnd` 3 seconds at most.
fn timeout(event: &str) -> u64 {
    if event == "SessionEnd" { 3 } else { 5 }
}

pub fn install(home: &Path) -> anyhow::Result<Installed> {
    let codex = folder("CODEX_HOME", home.join(".codex"));
    let path = codex.join("hooks.json");
    let (old, mut value) = files::read_json(&path)?;
    let added = groups::add(
        groups::hooks(&mut value)?,
        &EVENTS,
        |event| command_for("codex", event),
        |event, command| groups::group(command, timeout(event), false),
    )?;
    files::write_json(&path, old.as_deref(), &value)?;
    let config = codex.join("config.toml");
    let old = files::read(&config)?;
    let text = trusted(
        &features_on(old.as_deref().unwrap_or_default())?,
        &path,
        &value,
    );
    let text = super::title::install(&text)?;
    let changed = files::write(&config, old.as_deref(), &text)?;
    Ok(Installed {
        path,
        added: added + usize::from(changed),
    })
}

/// `config` with Codex's hooks feature on; an owner's `false` stops the install.
fn features_on(config: &str) -> anyhow::Result<String> {
    let mut table = "";
    let mut header = None;
    for (at, line) in config.lines().enumerate() {
        let line = line.trim();
        if line.starts_with('[') {
            table = line.trim_matches(['[', ']', ' ']);
            if table == "features" {
                header = Some(at);
            }
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let ours = (table == "features" && key == "hooks")
            || (table.is_empty() && key == "features.hooks");
        match value.split('#').next().map(str::trim) {
            Some("true") if ours => return Ok(config.into()),
            Some("false") if ours => anyhow::bail!(
                "Codex's features.hooks is false in its config.toml; set it to true for its hooks"
            ),
            _ => {}
        }
    }
    let mut lines: Vec<&str> = config.lines().collect();
    Ok(match header {
        Some(at) => {
            lines.insert(at + 1, "hooks = true");
            lines.join("\n") + "\n"
        }
        None => {
            let text = config.trim_end();
            let gap = if text.is_empty() { "" } else { "\n\n" };
            format!("{text}{gap}[features]\nhooks = true\n")
        }
    })
}

/// `config` with a trust entry for each of Ely Tea Party's hooks in `hooks` at `path`, as Codex would write it.
fn trusted(config: &str, path: &Path, hooks: &Value) -> String {
    let begin = format!("# {MARK} codex trust begin");
    let end = format!("# {MARK} codex trust end");
    // Codex may move unrelated tables between our old comment markers.
    let outside = config
        .split_inclusive('\n')
        .filter(|line| ![begin.as_str(), end.as_str()].contains(&line.trim()))
        .collect::<String>();
    let mut body = String::new();
    for event in EVENTS {
        let command = command_for("codex", event);
        let Some(groups) = hooks["hooks"][event].as_array() else {
            continue;
        };
        for (at, group) in groups.iter().enumerate() {
            let Some(handlers) = group["hooks"].as_array() else {
                continue;
            };
            for (index, handler) in handlers.iter().enumerate() {
                if handler["command"] != command.as_str() {
                    continue;
                }
                let label = snake(event);
                let key = format!("{}:{label}:{at}:{index}", path.display());
                let table = format!("[hooks.state.\"{key}\"]");
                // A trust Codex wrote itself stays; a second table of one name breaks the file.
                if outside.contains(&table) {
                    continue;
                }
                let hash = hash(&label, &command, timeout(event));
                body.push_str(&format!("{table}\ntrusted_hash = \"{hash}\"\n\n"));
            }
        }
    }
    if body.is_empty() {
        outside
    } else {
        format!("{}\n\n{}", outside.trim_end(), body)
    }
}

/// Codex's hash of one handler: sha256 of its compact JSON, keys sorted.
fn hash(label: &str, command: &str, timeout: u64) -> String {
    let text = |value: &str| Value::from(value).to_string();
    let json = format!(
        r#"{{"event_name":{},"hooks":[{{"async":false,"command":{},"timeout":{timeout},"type":"command"}}]}}"#,
        text(label),
        text(command),
    );
    format!("sha256:{:x}", Sha256::digest(json.as_bytes()))
}

/// `PreToolUse` as Codex labels it: `pre_tool_use`.
fn snake(event: &str) -> String {
    let mut label = String::new();
    for (at, c) in event.chars().enumerate() {
        if c.is_ascii_uppercase() && at > 0 {
            label.push('_');
        }
        label.push(c.to_ascii_lowercase());
    }
    label
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_feature_turns_on_once_and_an_owners_no_stands() {
        assert_eq!(features_on("").expect("on"), "[features]\nhooks = true\n");
        let other = "model = \"o\"\n[features]\nweb = true\n";
        let on = features_on(other).expect("on");
        assert_eq!(on, "model = \"o\"\n[features]\nhooks = true\nweb = true\n");
        assert_eq!(features_on(&on).expect("on"), on);
        assert_eq!(
            features_on("features.hooks = true\n").expect("on"),
            "features.hooks = true\n"
        );
        assert!(features_on("[features]\nhooks = false\n").is_err());
    }

    #[test]
    fn trust_matches_what_codex_hashes() {
        // The hash the rule that matched Codex's own trust entries gives, worked in Python.
        assert_eq!(
            hash("stop", r#"/bin/sh -c 'exit 0' "q""#, 5),
            "sha256:843300a4739551909234d70f3bde8e4b4477d327f96b90a22755a180ea80a7ca"
        );
        assert_eq!(snake("UserPromptSubmit"), "user_prompt_submit");
        let command = "other";
        let hooks = serde_json::json!({ "hooks": { "Stop": [
            { "hooks": [{ "command": command }] },
            { "hooks": [{ "command": command_for("codex", "Stop") }] },
        ] } });
        let path = Path::new("/h/.codex/hooks.json");
        let config = trusted("", path, &hooks);
        assert!(config.contains("[hooks.state.\"/h/.codex/hooks.json:stop:1:0\"]"));
        assert_eq!(
            trusted(&config, path, &hooks),
            config,
            "a second install changes nothing"
        );
        let owned = "[hooks.state.\"/h/.codex/hooks.json:stop:1:0\"]\ntrusted_hash = \"x\"\n";
        assert!(!trusted(owned, path, &hooks).contains("ely-tea-party codex trust begin\n["));
    }

    #[test]
    fn codex_reordered_preferences_inside_old_markers_survive_install() {
        let config = concat!(
            "service_tier = \"ultrafast\"\n",
            "# ely-tea-party codex trust begin\n",
            "[hooks.state.\"/h/.codex/hooks.json:stop:0:0\"]\n",
            "trusted_hash = \"owner\"\n",
            "[tui]\nterminal_title = []\n",
            "[notice]\nhide_rate_limit_model_nudge = true\n",
            "# ely-tea-party codex trust end\n",
        );
        let hooks = serde_json::json!({"hooks": {"Stop": [
            {"hooks": [{"command": command_for("codex", "Stop")}]}
        ]}});
        let path = Path::new("/h/.codex/hooks.json");
        let installed = super::super::title::install(&trusted(config, path, &hooks)).unwrap();
        let before: toml_edit::DocumentMut = config.parse().unwrap();
        let after: toml_edit::DocumentMut = installed.parse().unwrap();
        for key in ["service_tier", "hooks", "tui", "notice"] {
            assert_eq!(before[key].to_string(), after[key].to_string(), "{key}");
        }
        assert_eq!(trusted(&installed, path, &hooks), installed);
    }
}
