//! The prompts an agent's own record of a conversation holds, read when its session comes back.

use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use serde_json::Value;

/// A prompt, and the reply that ended its turn when the record says it.
pub type Turn = (String, Option<String>);

/// Reads one line of a record into the turns so far; the conversation's id is its second word.
type Read = fn(&str, &str, &mut Vec<Turn>);

/// The most characters kept of a prompt or a reply, which a tick's preview shows on one line.
const SHOWN: usize = 400;

/// The agent and conversation a resume line names, as [`crate::workspace::restore::resume`] writes it.
pub fn named(line: &str) -> Option<(&str, &str)> {
    let mut words = line.split_whitespace();
    let (agent, _flag, conversation) = (words.next()?, words.next()?, words.next()?);
    words.next().is_none().then_some((agent, conversation))
}

/// The prompts of `agent`'s `conversation`, oldest first; none for an agent whose record is not read, or a record not found.
pub fn turns(agent: &str, conversation: &str) -> Option<Vec<Turn>> {
    let home = dirs::home_dir()?;
    let folder = |variable: &str, under: &str| {
        std::env::var_os(variable).map_or_else(|| home.join(under), PathBuf::from)
    };
    let (path, read): (PathBuf, Read) = match agent {
        "claude" => {
            let projects = folder("CLAUDE_CONFIG_DIR", ".claude").join("projects");
            (
                within(&projects, &format!("{conversation}.jsonl"), 1)?,
                claude,
            )
        }
        "codex" => {
            let sessions = folder("CODEX_HOME", ".codex").join("sessions");
            (
                within(&sessions, &format!("-{conversation}.jsonl"), 3)?,
                codex,
            )
        }
        "agy" | "antigravity" => (
            home.join(".gemini/antigravity-cli/history.jsonl"),
            antigravity,
        ),
        _ => return None,
    };
    let file = File::open(&path)
        .map_err(|error| log::info!("transcripts: cannot read {}: {error}", path.display()))
        .ok()?;
    let mut turns = Vec::new();
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        read(&line, conversation, &mut turns);
    }
    log::info!(
        "transcripts: {} prompts of {agent}'s conversation in {}",
        turns.len(),
        path.display()
    );
    Some(turns)
}

/// The file under `folder`, at most `depth` folders down, whose name ends in `ending`.
fn within(folder: &Path, ending: &str, depth: usize) -> Option<PathBuf> {
    let mut below = Vec::new();
    for entry in std::fs::read_dir(folder).ok()?.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if name.to_string_lossy().ends_with(ending) && path.is_file() {
            return Some(path);
        }
        if depth > 0 && path.is_dir() {
            below.push(path);
        }
    }
    // The newest folders first: a session that comes back was used lately.
    below.sort();
    below
        .iter()
        .rev()
        .find_map(|folder| within(folder, ending, depth - 1))
}

fn cut(text: &str) -> String {
    text.trim().chars().take(SHOWN).collect()
}

fn prompt(turns: &mut Vec<Turn>, text: &str) {
    let text = cut(text);
    // What the agent's harness sends in the user's name opens with a tag.
    if text.is_empty() || text.starts_with('<') {
        return;
    }
    if turns.last().is_none_or(|(last, _)| *last != text) {
        turns.push((text, None));
    }
}

fn reply(turns: &mut [Turn], text: &str) {
    let text = cut(text);
    if let (Some(turn), false) = (turns.last_mut(), text.is_empty()) {
        turn.1 = Some(text);
    }
}

/// The text blocks of a message's content, joined; none when it carries a tool's result.
fn texts(content: &Value) -> Option<String> {
    match content {
        Value::String(text) => Some(text.clone()),
        Value::Array(blocks) => {
            let kind = |block: &Value| block["type"].as_str().unwrap_or_default().to_owned();
            if blocks.iter().any(|block| kind(block) == "tool_result") {
                return None;
            }
            let parts: Vec<&str> = blocks
                .iter()
                .filter_map(|block| block["text"].as_str())
                .collect();
            Some(parts.join("\n"))
        }
        _ => None,
    }
}

/// Claude Code's session file: a line a message, the user's own neither meta nor a subagent's.
fn claude(line: &str, _: &str, turns: &mut Vec<Turn>) {
    if !line.contains("\"user\"") && !line.contains("\"assistant\"") {
        return;
    }
    let Ok(entry) = serde_json::from_str::<Value>(line) else {
        return;
    };
    let flag = |name: &str| entry[name].as_bool().unwrap_or(false);
    if flag("isMeta") || flag("isSidechain") || flag("isCompactSummary") {
        return;
    }
    let Some(text) = texts(&entry["message"]["content"]) else {
        return;
    };
    match entry["type"].as_str() {
        Some("user") => prompt(turns, &text),
        Some("assistant") => reply(turns, &text),
        _ => {}
    }
}

/// Codex's rollout file: its events name each user message and each turn's last words.
fn codex(line: &str, _: &str, turns: &mut Vec<Turn>) {
    if !line.contains("\"event_msg\"") {
        return;
    }
    let Ok(entry) = serde_json::from_str::<Value>(line) else {
        return;
    };
    let event = &entry["payload"];
    match event["type"].as_str() {
        Some("item_completed") if event["item"]["type"] == "UserMessage" => {
            if let Some(text) = texts(&event["item"]["content"]) {
                prompt(turns, &text);
            }
        }
        Some("user_message") => prompt(turns, event["message"].as_str().unwrap_or_default()),
        Some("task_complete") => reply(
            turns,
            event["last_agent_message"].as_str().unwrap_or_default(),
        ),
        _ => {}
    }
}

/// Antigravity's input history: every conversation's prompts in one file, commands among them.
fn antigravity(line: &str, conversation: &str, turns: &mut Vec<Turn>) {
    if !line.contains(conversation) {
        return;
    }
    let Ok(entry) = serde_json::from_str::<Value>(line) else {
        return;
    };
    let shown = entry["display"].as_str().unwrap_or_default();
    if entry["conversationId"] == conversation && !shown.trim_start().starts_with('/') {
        prompt(turns, shown);
    }
}

#[cfg(test)]
mod tests;
