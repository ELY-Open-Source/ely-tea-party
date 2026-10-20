//! What a hook's payload says, in Claude Code's terms, whichever agent wrote it.

use serde_json::Value;

/// The longest detail a report carries, in characters.
pub const DETAIL: usize = 80;

/// Agents whose `SessionEnd` ends a turn, not the session.
const TURN_ENDERS: [&str; 3] = ["grok", "agy", "antigravity"];

/// What a hook reports: its event in Claude Code's name, the tool, and what it touches.
#[derive(Debug, PartialEq)]
pub struct Said {
    pub event: String,
    pub tool: Option<String>,
    pub detail: Option<String>,
    pub conversation: Option<String>,
}

/// Reads `agent`'s payload; `named` is the event its command line gave. None: the event means nothing here.
pub fn read(agent: &str, named: Option<&str>, payload: &Value) -> Option<Said> {
    let event = named.or_else(|| {
        [
            "hook_event_name",
            "hookEventName",
            "event_name",
            "event",
            "type",
        ]
        .iter()
        .find_map(|key| payload[key].as_str())
    })?;
    let event = canonical(agent, event, payload)?;
    let tool = ["tool_name", "toolName"]
        .iter()
        .find_map(|key| payload[key].as_str())
        .or_else(|| payload["toolCall"]["name"].as_str())
        .map(str::to_string);
    let ids = [
        "session_id",
        "sessionId",
        "conversation_id",
        "conversationId",
    ];
    Some(Said {
        event: event.to_string(),
        tool,
        detail: detail(payload),
        conversation: field(&ids, payload).map(str::to_string),
    })
}

/// `event` as Claude Code names it; None for one to drop.
fn canonical<'a>(agent: &str, event: &'a str, payload: &Value) -> Option<&'a str> {
    Some(match event {
        "BeforeAgent" | "PreInvocation" | "user_prompt_submit" => "UserPromptSubmit",
        "AfterAgent" | "turn-completion" | "stop" => "Stop",
        "BeforeTool" | "pre_tool_use" => "PreToolUse",
        "AfterTool" | "post_tool_use" => "PostToolUse",
        "PreCompress" | "pre_compact" => "PreCompact",
        "permission_request" => "PermissionRequest",
        "session_start" => "SessionStart",
        "subagent_start" => "SubagentStart",
        "subagent_stop" => "SubagentStop",
        "SessionEnd" | "session_end" if TURN_ENDERS.contains(&agent) => "Stop",
        "session_end" => "SessionEnd",
        // Grok's notifications are mostly a finished turn's text; only a prompt asks.
        "Notification" if agent == "grok" => {
            let kind = payload["notificationType"].as_str();
            kind.filter(|kind| *kind == "permission_prompt")
                .map(|_| "Notification")?
        }
        other => other,
    })
}

/// What the event works on or says, on one line: a file's name, a command, a pattern, a message.
fn detail(payload: &Value) -> Option<String> {
    let input = ["tool_input", "toolInput"]
        .iter()
        .map(|key| &payload[*key])
        .find(|input| input.is_object())
        .or(Some(&payload["toolCall"]["args"]))
        .unwrap_or(&Value::Null);
    let name = |path: &str| path.rsplit(['/', '\\']).next().unwrap_or(path).to_string();
    let found = field(
        &["file_path", "filePath", "absolute_path", "notebook_path"],
        input,
    )
    .map(name)
    .or_else(|| field(&["command", "cmd", "pattern", "query", "url"], input).map(str::to_string))
    .or_else(|| field(&["path"], input).map(name))
    .or_else(|| field(&["description"], input).map(str::to_string))
    .or_else(|| {
        let keys = [
            "prompt",
            "message",
            "last_assistant_message",
            "last-assistant-message",
        ];
        field(&keys, payload).map(str::to_string)
    })?;
    let line = found.lines().map(str::trim).find(|line| !line.is_empty())?;
    let mut clipped: String = line.chars().take(DETAIL).collect();
    if line.chars().nth(DETAIL).is_some() {
        clipped.push('…');
    }
    Some(clipped)
}

/// The first of `keys` that `from` holds as text.
fn field<'a>(keys: &[&str], from: &'a Value) -> Option<&'a str> {
    keys.iter().find_map(|key| from[key].as_str())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn said(agent: &str, named: Option<&str>, payload: Value) -> Option<Said> {
        read(agent, named, &payload)
    }

    #[test]
    fn a_payload_tells_the_tool_and_what_it_touches() {
        let read = json!({ "hook_event_name": "PreToolUse", "tool_name": "Read", "tool_input": { "file_path": "/a/src/links.rs" } });
        let read = said("claude", None, read).expect("an event");
        assert_eq!(read.tool.as_deref(), Some("Read"));
        assert_eq!(read.detail.as_deref(), Some("links.rs"));
        let bash = json!({ "type": "PreToolUse", "tool_input": { "command": "\n cargo test\nmore", "description": "Run" } });
        let bash = said("claude", None, bash).expect("an event");
        assert_eq!(bash.detail.as_deref(), Some("cargo test"));
        let grep = json!({ "event": "PreToolUse", "tool_input": { "pattern": "link_at", "path": "/a/src" } });
        let grep = said("claude", None, grep).expect("an event");
        assert_eq!(grep.detail.as_deref(), Some("link_at"));
        let long = json!({ "type": "x", "tool_input": { "pattern": "x".repeat(100) } });
        let long = said("claude", None, long).and_then(|said| said.detail);
        assert_eq!(long.map(|text| text.chars().count()), Some(DETAIL + 1));
        let asked = json!({ "hook_event_name": "UserPromptSubmit", "prompt": "\nfix it\nnow" });
        let asked = said("claude", None, asked).and_then(|said| said.detail);
        assert_eq!(asked.as_deref(), Some("fix it"));
        assert_eq!(said("claude", None, json!({})), None, "no event");
    }

    #[test]
    fn other_agents_speak_in_claude_codes_names() {
        let gemini = json!({ "hook_event_name": "BeforeTool", "tool_name": "read_file", "tool_input": { "absolute_path": "/a/b.rs" } });
        let gemini = said("gemini", None, gemini).expect("an event");
        assert_eq!(gemini.event, "PreToolUse");
        assert_eq!(gemini.detail.as_deref(), Some("b.rs"));
        let opencode = json!({ "tool_name": "read", "tool_input": { "filePath": "/a/c.rs" } });
        let opencode = said("opencode", Some("PreToolUse"), opencode).expect("an event");
        assert_eq!(opencode.detail.as_deref(), Some("c.rs"));
        let grok = json!({ "hookEventName": "Notification", "notificationType": "permission_prompt", "message": "Allow?" });
        assert_eq!(
            said("grok", None, grok).map(|said| said.event).as_deref(),
            Some("Notification")
        );
        let done =
            json!({ "notificationType": "turn_complete", "message": "Turn complete in 1.2s." });
        assert_eq!(
            said("grok", Some("Notification"), done),
            None,
            "a finished turn's text"
        );
        let turn = said("grok", Some("SessionEnd"), json!({})).map(|said| said.event);
        assert_eq!(turn.as_deref(), Some("Stop"), "Grok ends each turn so");
        let end = said("claude", Some("SessionEnd"), json!({})).map(|said| said.event);
        assert_eq!(end.as_deref(), Some("SessionEnd"));
        let codex = said("codex", None, json!({ "hook_event_name": "pre_tool_use" }));
        assert_eq!(codex.map(|said| said.event).as_deref(), Some("PreToolUse"));
        let agy = said("agy", Some("PreInvocation"), json!({})).map(|said| said.event);
        assert_eq!(agy.as_deref(), Some("UserPromptSubmit"));
        let asks = json!({ "type": "Notification", "message": "Claude needs your permission to use Bash" });
        let asks = said("claude", None, asks).and_then(|said| said.detail);
        assert_eq!(
            asks.as_deref(),
            Some("Claude needs your permission to use Bash")
        );
    }
}
