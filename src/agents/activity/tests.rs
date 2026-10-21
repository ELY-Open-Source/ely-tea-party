use std::time::{Duration, Instant};

use super::*;
use crate::agents::{Kind, timeline::Timeline};

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

/// A hooked agent that heard `event` at `at`, as the workspace hands it on.
fn hear(
    agent: &mut Agent,
    (event, status): (&str, Status),
    tool: Option<&str>,
    detail: Option<&str>,
    at: Instant,
) {
    agent.hooked = true;
    agent.note(event, (tool, detail), &mut Timeline::default(), at);
    agent.set(status, at);
}

#[test]
fn tools_choose_the_pose_and_the_row_says_what_they_touch() {
    let start = Instant::now();
    let mut agent = Agent::new(Kind::Claude, start);
    hear(
        &mut agent,
        ("UserPromptSubmit", Status::Working),
        None,
        None,
        start,
    );
    assert_eq!(agent.pose(start + secs(1), 0), Pose::Thinking);
    assert_eq!(agent.news(Pose::Thinking), None);
    hear(
        &mut agent,
        ("PreToolUse", Status::Working),
        Some("Read"),
        Some("links.rs"),
        start,
    );
    assert_eq!(agent.pose(start + secs(1), 0), Pose::Research);
    assert_eq!(agent.news(Pose::Research).as_deref(), Some("读 links.rs"));
    hear(
        &mut agent,
        ("PostToolUse", Status::Working),
        Some("Read"),
        None,
        start,
    );
    assert_eq!(agent.pose(start + secs(2), 0), Pose::Research, "just back");
    assert_eq!(
        agent.pose(start + secs(7), 0),
        Pose::Thinking,
        "then it thinks"
    );
    hear(
        &mut agent,
        ("PreToolUse", Status::Working),
        Some("Task"),
        Some("Find the bug"),
        start,
    );
    assert_eq!(agent.pose(start, 0), Pose::Special, "subagents fly");
    hear(
        &mut agent,
        ("PreToolUse", Status::Working),
        Some("mcp__x__y"),
        None,
        start,
    );
    assert_eq!(agent.news(Pose::Working).as_deref(), Some("用 mcp__x__y"));
}

#[test]
fn a_long_command_lets_her_slack_until_it_ends() {
    let start = Instant::now();
    let mut agent = Agent::new(Kind::Deepseek, start);
    hear(
        &mut agent,
        ("PreToolUse", Status::Working),
        Some("Bash"),
        Some("cargo test"),
        start,
    );
    assert_eq!(agent.pose(start + secs(5), 0), Pose::Working);
    assert_eq!(agent.pose(start + secs(25), 0), Pose::Slacking);
    assert_eq!(
        agent.news(Pose::Slacking).as_deref(),
        Some("等 cargo test 跑完")
    );
    hear(
        &mut agent,
        ("PostToolUse", Status::Working),
        Some("Bash"),
        None,
        start + secs(30),
    );
    assert_eq!(agent.pose(start + secs(31), 0), Pose::Working);
}

#[test]
fn a_long_turn_ends_in_a_finale_and_a_count_of_edits() {
    let start = Instant::now();
    let mut agent = Agent::new(Kind::Pi, start);
    hear(
        &mut agent,
        ("UserPromptSubmit", Status::Working),
        None,
        None,
        start,
    );
    for file in ["a.rs", "b.rs", "a.rs"] {
        hear(
            &mut agent,
            ("PreToolUse", Status::Working),
            Some("Edit"),
            Some(file),
            start,
        );
    }
    hear(
        &mut agent,
        ("Stop", Status::Idle),
        None,
        None,
        start + secs(40),
    );
    assert_eq!(agent.pose(start + secs(41), 0), Pose::Special);
    assert_eq!(agent.pose(start + secs(50), 0), Pose::Idle);
    assert_eq!(agent.news(Pose::Idle).as_deref(), Some("这轮改了 2 个文件"));
    assert_eq!(agent.news(Pose::Sleeping), None);
    hear(
        &mut agent,
        ("UserPromptSubmit", Status::Working),
        None,
        None,
        start + secs(60),
    );
    hear(
        &mut agent,
        ("Stop", Status::Idle),
        None,
        None,
        start + secs(61),
    );
    assert_eq!(
        agent.pose(start + secs(62), 0),
        Pose::Idle,
        "a short turn, no finale"
    );
}

#[test]
fn a_question_shows_what_she_asks() {
    let start = Instant::now();
    let mut agent = Agent::new(Kind::Gpt, start);
    let asks = "Claude needs your permission to use Bash";
    hear(
        &mut agent,
        ("Notification", Status::NeedsInput),
        None,
        Some(asks),
        start,
    );
    assert_eq!(agent.pose(start, 0), Pose::NeedsInput);
    assert_eq!(agent.news(Pose::NeedsInput).as_deref(), Some(asks));
    hear(&mut agent, ("Stop", Status::Idle), None, None, start);
    assert_eq!(agent.news(Pose::Idle), None, "an answered question is gone");
}

#[test]
fn every_agent_spells_its_tools_its_own_way() {
    assert_eq!(Doing::of("Read"), Doing::Reading);
    assert_eq!(Doing::of("read_file"), Doing::Reading, "Gemini");
    assert_eq!(Doing::of("StrReplaceFile"), Doing::Editing, "Kimi");
    assert_eq!(Doing::of("bash"), Doing::Running, "OpenCode and Pi");
    assert_eq!(Doing::of("run_shell_command"), Doing::Running, "Gemini");
    assert_eq!(Doing::of("mcp__x__y"), Doing::Other);
}

#[test]
fn a_tool_named_only_afterwards_still_shows() {
    let start = Instant::now();
    let mut agent = Agent::new(Kind::Gemini, start);
    hear(
        &mut agent,
        ("UserPromptSubmit", Status::Working),
        None,
        None,
        start,
    );
    hear(
        &mut agent,
        ("PostToolUse", Status::Working),
        Some("view_file"),
        Some("x.rs"),
        start,
    );
    assert_eq!(agent.pose(start + secs(1), 0), Pose::Research);
    assert_eq!(agent.news(Pose::Research).as_deref(), Some("读 x.rs"));
    assert_eq!(
        agent.pose(start + secs(7), 0),
        Pose::Thinking,
        "it already ended"
    );
    assert_eq!(agent.turn.tools, 1);
}

#[test]
fn she_speaks_at_the_moments_that_matter_then_falls_quiet() {
    use crate::agents::quips::Moment;
    let start = Instant::now();
    let mut agent = Agent::new(Kind::Grok, start);
    hear(
        &mut agent,
        ("UserPromptSubmit", Status::Working),
        None,
        None,
        start,
    );
    assert_eq!(
        agent.bubble(start).map(|said| said.0),
        Some(Moment::Received)
    );
    assert_eq!(agent.bubble(start + secs(7)), None, "a word fades");
    hear(
        &mut agent,
        ("PreToolUse", Status::Working),
        Some("Bash"),
        Some("cargo test"),
        start,
    );
    hear(
        &mut agent,
        ("PostToolUse", Status::Working),
        Some("Bash"),
        None,
        start + secs(30),
    );
    assert_eq!(
        agent.bubble(start + secs(30)).map(|said| said.0),
        Some(Moment::LongDone)
    );
    hear(
        &mut agent,
        ("PreToolUse", Status::Working),
        Some("Bash"),
        Some("false"),
        start + secs(31),
    );
    hear(
        &mut agent,
        ("PostToolUseFailure", Status::Working),
        Some("Bash"),
        None,
        start + secs(32),
    );
    assert_eq!(
        agent.bubble(start + secs(32)).map(|said| said.0),
        Some(Moment::Failed)
    );
    hear(
        &mut agent,
        ("Stop", Status::Idle),
        None,
        None,
        start + secs(40),
    );
    assert_eq!(
        agent.bubble(start + secs(40)).map(|said| said.0),
        Some(Moment::Finale)
    );
}
