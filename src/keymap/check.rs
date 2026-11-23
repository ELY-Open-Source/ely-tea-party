//! Whether the settings' shortcuts can all be bound.

use anyhow::{Context as _, bail, ensure};
use gpui::Keystroke;

use super::{Command, FOCUS_KEYS, MODIFIERS, Shortcuts, commands};

/// Refuses a command no table names, a key that does not read, and a key two commands of one window hold.
pub fn check(shortcuts: &Shortcuts) -> anyhow::Result<()> {
    let commands = commands();
    for name in shortcuts.keys() {
        ensure!(
            commands.iter().any(|command| command.name == *name),
            "shortcuts names {name}, which is no command"
        );
    }
    let mut held: Vec<(String, &Command)> = Vec::new();
    for command in &commands {
        for key in command.keys(shortcuts, &commands) {
            let stroke = Keystroke::parse(&key)
                .with_context(|| format!("shortcuts: {}'s {key:?} is no key", command.name))?;
            // gpui reads any word as a key's name.
            let named = !stroke.key.is_empty()
                && !stroke.key.contains(char::is_whitespace)
                && !MODIFIERS.contains(&stroke.key.as_str());
            ensure!(named, "shortcuts: {}'s {key:?} names no key", command.name);
            let stroke = stroke.unparse();
            ensure!(
                !FOCUS_KEYS.contains(&stroke.as_str()),
                "shortcuts: {key} moves the focus and goes to the terminal's program, so {} cannot take it",
                command.name
            );
            let taken = held
                .iter()
                .find(|(other, holder)| *other == stroke && holder.meets(command));
            if let Some((_, holder)) = taken {
                bail!(
                    "shortcuts: {key} runs {} already, so {} cannot take it",
                    holder.name,
                    command.name
                );
            }
            held.push((stroke, command));
        }
    }
    Ok(())
}
