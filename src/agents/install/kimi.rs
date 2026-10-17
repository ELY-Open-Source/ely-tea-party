//! Kimi Code's hooks: `[[hooks]]` tables in its config.toml, in a block Ely Tea Party owns.

use std::path::Path;

use super::{Installed, MARK, command_for, files, folder};

const EVENTS: [&str; 8] = [
    "SessionStart",
    "UserPromptSubmit",
    "Notification",
    "Stop",
    "StopFailure",
    "SessionEnd",
    "PreToolUse",
    "PostToolUse",
];

pub fn install(home: &Path) -> anyhow::Result<Installed> {
    let current = folder("KIMI_CODE_HOME", home.join(".kimi-code")).join("config.toml");
    let legacy = folder("KIMI_SHARE_DIR", home.join(".kimi")).join("config.toml");
    let path = if !current.exists() && legacy.exists() {
        legacy
    } else {
        current
    };
    let old = files::read(&path)?;
    let text = with_hooks(old.as_deref().unwrap_or_default());
    let wrote = files::write(&path, old.as_deref(), &text)?;
    Ok(Installed {
        path,
        added: if wrote { EVENTS.len() } else { 0 },
    })
}

/// `config` with Ely Tea Party's block of hooks.
fn with_hooks(config: &str) -> String {
    let body: String = EVENTS
        .iter()
        .map(|event| {
            let command = files::toml_string(&command_for("kimi", event));
            format!("[[hooks]]\nevent = \"{event}\"\ncommand = {command}\ntimeout = 5\n\n")
        })
        .collect();
    let begin = format!("# {MARK} hooks begin");
    let end = format!("# {MARK} hooks end");
    files::with_block(config, (&begin, &end), &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_block_lands_once_after_the_owners_settings() {
        let config = "default_model = \"k3\"\n";
        let once = with_hooks(config);
        assert!(once.starts_with(config));
        assert_eq!(once.matches("[[hooks]]").count(), EVENTS.len());
        assert!(once.contains("event = \"PreToolUse\"\ncommand = \"h=$(printenv ELY_HOOK);"));
        assert_eq!(with_hooks(&once), once, "a second install changes nothing");
    }
}
