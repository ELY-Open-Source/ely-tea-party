//! Claude's status line forwards session metadata and preserves the owner's output.

use serde_json::{Value, json};

const MARK: &str = "# ely-tea-party speed";

pub fn install(settings: &mut Value) -> anyhow::Result<usize> {
    let previous = settings.get("statusLine");
    if previous
        .and_then(|value| value["command"].as_str())
        .is_some_and(|command| command.ends_with(MARK))
    {
        return Ok(0);
    }
    if let Some(previous) = previous {
        anyhow::ensure!(
            previous["type"] == "command" && previous["command"].is_string(),
            "install: Claude statusLine is not a command; cannot preserve it"
        );
    }
    let mut status = previous
        .cloned()
        .unwrap_or_else(|| json!({"type": "command"}));
    let original = status["command"].as_str().unwrap_or(":");
    let quoted = original.replace('\'', "'\\''");
    let command = format!(
        "ely_input=$(cat; printf '.')\n\
         ely_input=${{ely_input%.}}\n\
         if [ -n \"$ELY_HOOK\" ]; then\n\
         printf '%s' \"$ely_input\" | \"$ELY_HOOK\" hook claude Speed 2>/dev/null || true\n\
         fi\n\
         printf '%s' \"$ely_input\" | sh -c '{quoted}'\n\
         {MARK}"
    );
    status["command"] = Value::String(command);
    settings["statusLine"] = status;
    Ok(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        process::{Command, Stdio},
    };

    #[test]
    fn the_owners_command_gets_the_same_input_and_output() {
        let mut settings = json!({
            "statusLine": {"type": "command", "command": "printf 'owner:'; cat", "padding": 2}
        });
        assert_eq!(install(&mut settings).unwrap(), 1);
        assert_eq!(install(&mut settings).unwrap(), 0);
        assert_eq!(settings["statusLine"]["padding"], 2);
        let mut child = Command::new("sh")
            .args(["-c", settings["statusLine"]["command"].as_str().unwrap()])
            .env_remove("ELY_HOOK")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"{\"fast_mode\":true}\n\n")
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"owner:{\"fast_mode\":true}\n\n");
    }

    #[test]
    fn unknown_status_line_shapes_are_not_overwritten() {
        let mut settings = json!({"statusLine": {"type": "future"}});
        let before = settings.clone();
        assert!(install(&mut settings).is_err());
        assert_eq!(settings, before);
    }
}
