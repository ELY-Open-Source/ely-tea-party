use super::*;

fn read(parse: fn(&str, &str, &mut Vec<Turn>), lines: &[&str]) -> Vec<Turn> {
    let mut turns = Vec::new();
    for line in lines {
        parse(line, "c1", &mut turns);
    }
    turns
}

#[test]
fn a_resume_line_names_its_agent_and_conversation() {
    assert_eq!(named("claude --resume a1-b2"), Some(("claude", "a1-b2")));
    assert_eq!(named("codex resume 019a"), Some(("codex", "019a")));
    assert_eq!(named("claude"), None);
    assert_eq!(named("claude --resume a b"), None);
}

#[test]
fn claude_code_s_prompts_leave_out_what_its_harness_sends() {
    let turns = read(
        claude,
        &[
            r#"{"type":"user","message":{"role":"user","content":"<command-name>/login</command-name>"}}"#,
            r#"{"type":"user","message":{"role":"user","content":"fix the build"}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"hm"}]}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Looking."},{"type":"tool_use","name":"Bash"}]}}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":"ok"}]}}"#,
            r#"{"type":"user","isSidechain":true,"message":{"role":"user","content":"a subagent's task"}}"#,
            r#"{"type":"user","isMeta":true,"message":{"role":"user","content":"[Image: original 2x2]"}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Fixed."}]}}"#,
            r#"{"type":"user","message":{"role":"user","content":"<task-notification>done</task-notification>"}}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"[Image #1] and this?"},{"type":"image","source":{}}]}}"#,
            "not json \"user\"",
        ],
    );
    assert_eq!(
        turns,
        [
            ("fix the build".to_string(), Some("Fixed.".to_string())),
            ("[Image #1] and this?".to_string(), None),
        ]
    );
}

#[test]
fn codex_s_events_name_each_prompt_once_and_its_turn_s_last_words() {
    let turns = read(
        codex,
        &[
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<environment_context>"}]}}"#,
            r#"{"type":"event_msg","payload":{"type":"user_message","message":"draw a whale"}}"#,
            r#"{"type":"event_msg","payload":{"type":"item_completed","item":{"type":"UserMessage","content":[{"type":"text","text":"draw a whale"}]}}}"#,
            r#"{"type":"event_msg","payload":{"type":"item_completed","item":{"type":"AgentMessage","content":[{"type":"text","text":"on it"}]}}}"#,
            r#"{"type":"event_msg","payload":{"type":"task_complete","last_agent_message":"Saved as whale.png"}}"#,
            r#"{"type":"event_msg","payload":{"type":"item_completed","item":{"type":"UserMessage","content":[{"type":"text","text":"bigger"}]}}}"#,
        ],
    );
    assert_eq!(
        turns,
        [
            (
                "draw a whale".to_string(),
                Some("Saved as whale.png".to_string())
            ),
            ("bigger".to_string(), None),
        ]
    );
}

#[test]
fn antigravity_s_history_gives_one_conversation_s_prompts_without_its_commands() {
    let turns = read(
        antigravity,
        &[
            r#"{"display":"research the protocol","timestamp":1,"conversationId":"c1"}"#,
            r#"{"display":"another conversation's","timestamp":2,"conversationId":"c2"}"#,
            r#"{"display":"/browser","timestamp":3,"conversationId":"c1"}"#,
            r#"{"display":"draft it","timestamp":4,"conversationId":"c1"}"#,
        ],
    );
    let prompts: Vec<&str> = turns.iter().map(|(prompt, _)| prompt.as_str()).collect();
    assert_eq!(prompts, ["research the protocol", "draft it"]);
}

#[test]
fn a_record_is_found_by_its_name_s_ending_newest_folder_first() {
    let root = std::env::temp_dir().join(format!("ely-transcripts-{}", std::process::id()));
    let (old, new) = (root.join("2026/10/01"), root.join("2026/11/13"));
    for folder in [&old, &new] {
        std::fs::create_dir_all(folder).expect("a folder");
    }
    std::fs::write(old.join("rollout-a-c1.jsonl"), "").expect("a file");
    std::fs::write(new.join("rollout-b-c1.jsonl"), "").expect("a file");
    let found = within(&root, "-c1.jsonl", 3);
    assert_eq!(found, Some(new.join("rollout-b-c1.jsonl")));
    assert_eq!(within(&root, "-c1.jsonl", 2), None, "too deep");
    assert_eq!(within(&root, "-c9.jsonl", 3), None);
    std::fs::remove_dir_all(&root).expect("removed");
}
