//! Antigravity's plan limits, from its own `agy -p /usage`, which answers from the login it keeps and spends no quota.

use std::{
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime},
};

use anyhow::Context as _;
use serde::Deserialize;

use super::{Limit, Usage, window};

/// How long `agy` has to answer.
const PATIENCE: Duration = Duration::from_secs(20);
/// A session's hook variables, which the reading must not report under.
const SESSION_VARIABLES: [&str; 4] = ["ELY_SESSION", "ELY_SOCKET", "ELY_HOOK", "ELY_DRAWER"];

#[derive(Deserialize)]
struct Answer {
    status: Option<String>,
    command: Option<Asked>,
}

#[derive(Deserialize)]
struct Asked {
    data: Data,
}

#[derive(Deserialize)]
struct Data {
    #[serde(default)]
    groups: Vec<Group>,
}

#[derive(Deserialize)]
struct Group {
    name: String,
    #[serde(default)]
    buckets: Vec<Bucket>,
}

#[derive(Deserialize)]
struct Bucket {
    name: String,
    window: Option<String>,
    /// Left out when none remains.
    #[serde(default)]
    remaining_fraction: f32,
    reset_time: Option<String>,
}

pub fn read() -> anyhow::Result<Usage> {
    usage(&ask()?)
}

/// `agy`'s answer, run from the place its installer puts it, since an app started from the Dock has a bare `PATH`, else from `PATH`.
fn ask() -> anyhow::Result<Vec<u8>> {
    let installed = dirs::home_dir().map(|home| home.join(".local/bin/agy"));
    let program = installed
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from("agy"));
    let mut agy = Command::new(program);
    agy.args(["-p", "/usage", "--output-format", "json"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for variable in SESSION_VARIABLES {
        agy.env_remove(variable);
    }
    if let Some(home) = dirs::home_dir() {
        agy.current_dir(home);
    }
    let mut child = agy.spawn().context("usage: cannot run agy")?;
    let began = Instant::now();
    // Its answer is far smaller than a pipe holds, so it never waits on us.
    while child.try_wait()?.is_none() {
        if began.elapsed() >= PATIENCE {
            child.kill().ok();
            child.wait().ok();
            anyhow::bail!("usage: agy did not answer in time");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        let said = String::from_utf8_lossy(&output.stderr);
        let said = said.lines().next().unwrap_or("no reason given");
        anyhow::bail!("usage: agy refused: {said}");
    }
    Ok(output.stdout)
}

/// The Gemini group's limits in `answer`, the shorter window first; the owner reads only those.
fn usage(answer: &[u8]) -> anyhow::Result<Usage> {
    let answer: Answer =
        serde_json::from_slice(answer).context("usage: agy's answer does not parse")?;
    if let Some(status) = answer.status.filter(|status| status != "SUCCESS") {
        anyhow::bail!("usage: agy answered {status}");
    }
    let groups = answer
        .command
        .map_or_else(Vec::new, |asked| asked.data.groups);
    let mut buckets = groups
        .into_iter()
        .find(|group| group.name.starts_with("Gemini"))
        .map_or_else(Vec::new, |group| group.buckets);
    anyhow::ensure!(!buckets.is_empty(), "usage: agy names no Gemini limits");
    buckets.sort_by_key(|bucket| bucket.window.as_deref() != Some("5h"));
    Ok(Usage {
        plan: Some("Gemini".into()),
        limits: buckets.iter().map(limit).collect(),
        credits: None,
        note: None,
        banked: Vec::new(),
    })
}

fn limit(bucket: &Bucket) -> Limit {
    let span = match bucket.window.as_deref() {
        Some("5h") => window(300),
        Some("weekly") => window(10080),
        _ => bucket.name.clone(),
    };
    let resets = bucket.reset_time.as_deref().and_then(|at| {
        let at = chrono::DateTime::parse_from_rfc3339(at).ok()?;
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(u64::try_from(at.timestamp()).ok()?))
    });
    Limit {
        label: span,
        percent: ((1.0 - bucket.remaining_fraction) * 100.0).clamp(0.0, 100.0),
        resets,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_gemini_s_limits_read_short_window_first_as_the_share_used() {
        let usage = usage(
            br#"{"conversation_id":"","status":"SUCCESS","response":"x","command":{"name":"usage","data":{"description":"d","groups":[
                {"name":"Claude and GPT models","buckets":[
                    {"id":"3p-5h","name":"Five Hour Limit Remaining","window":"5h","remaining_fraction":0.5}]},
                {"name":"Gemini Models","buckets":[
                    {"id":"gemini-weekly","name":"Weekly Limit Remaining","window":"weekly","remaining_fraction":0.75,"reset_time":"2026-11-15T03:16:27Z"},
                    {"id":"gemini-5h","name":"Five Hour Limit Remaining","window":"5h"}]}
            ]}}}"#,
        )
        .expect("limits");
        let read: Vec<(&str, f32)> = (usage.limits.iter())
            .map(|limit| (limit.label.as_str(), limit.percent))
            .collect();
        assert_eq!(
            read,
            [("5 小时", 100.0), ("本周", 25.0)],
            "a bucket with no fraction has none left"
        );
        assert_eq!(usage.plan.as_deref(), Some("Gemini"));
        let reset = usage.limits[1].resets.expect("a reset");
        let secs = reset.duration_since(SystemTime::UNIX_EPOCH).expect("later");
        assert_eq!(secs.as_secs(), 1_794_712_587);
        assert_eq!(usage.limits[0].resets, None);
    }

    #[test]
    fn an_answer_with_no_limits_says_so() {
        let said = |answer: &[u8]| usage(answer).expect_err("no usage").to_string();
        assert_eq!(said(br#"{"status":"ERROR"}"#), "usage: agy answered ERROR");
        assert_eq!(
            said(br#"{"status":"SUCCESS"}"#),
            "usage: agy names no Gemini limits"
        );
        assert_eq!(said(b"Please log in"), "usage: agy's answer does not parse");
    }
}
