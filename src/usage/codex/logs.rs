//! Codex writes its plan's limits into each turn's `token_count` event in its session logs, so they are read there when the endpoint cannot be asked: as they were at the last turn this computer ran.

use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use anyhow::Context as _;
use serde::Deserialize;

use super::{Credits, credits};
use crate::usage::{Limit, Usage, window};

/// How much of a log's end is searched for its last limits; a log runs to tens of megabytes.
const TAIL: u64 = 2 << 20;

/// How many of the newest logs are tried, since a log just begun holds no turn yet.
const LOGS: usize = 8;

#[derive(Deserialize)]
struct Line {
    timestamp: Option<String>,
    payload: Payload,
}

#[derive(Deserialize)]
struct Payload {
    rate_limits: Option<Limits>,
}

#[derive(Deserialize)]
struct Limits {
    primary: Option<Window>,
    secondary: Option<Window>,
    credits: Option<Credits>,
    plan_type: Option<String>,
}

#[derive(Deserialize)]
struct Window {
    used_percent: f32,
    window_minutes: Option<u64>,
    resets_at: Option<u64>,
}

/// The limits the newest turn in Codex's logs under `home` reported, and when it did.
pub fn read(home: &Path) -> anyhow::Result<(Usage, Option<SystemTime>)> {
    let logs = newest(&home.join("sessions"))?;
    for log in &logs {
        if let Some((limits, at)) = last_limits(log)? {
            return Ok((usage(limits), at));
        }
    }
    anyhow::bail!(
        "usage: none of Codex's {} newest logs holds its limits yet",
        logs.len()
    )
}

/// The newest logs under `sessions`, which Codex files by year, month and day.
fn newest(sessions: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut logs = Vec::new();
    let mut days = Vec::new();
    for year in folders(sessions)?.into_iter().rev() {
        for month in folders(&year)?.into_iter().rev() {
            days.extend(folders(&month)?.into_iter().rev());
        }
        if days.len() >= 2 {
            break;
        }
    }
    for day in days.into_iter().take(2) {
        for entry in std::fs::read_dir(&day)? {
            let path = entry?.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "jsonl")
            {
                let changed = std::fs::metadata(&path)?.modified()?;
                logs.push((changed, path));
            }
        }
    }
    logs.sort_by_key(|(changed, _)| std::cmp::Reverse(*changed));
    Ok(logs.into_iter().take(LOGS).map(|(_, path)| path).collect())
}

/// `folder`'s subfolders in name order.
fn folders(folder: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let entries = std::fs::read_dir(folder)
        .with_context(|| format!("usage: cannot read {}", folder.display()))?;
    let mut folders = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            folders.push(entry.path());
        }
    }
    folders.sort();
    Ok(folders)
}

/// The limits `log`'s last turn reported, from its end, and when.
fn last_limits(log: &Path) -> anyhow::Result<Option<(Limits, Option<SystemTime>)>> {
    let mut file =
        File::open(log).with_context(|| format!("usage: cannot open {}", log.display()))?;
    let length = file.metadata()?.len();
    file.seek(SeekFrom::Start(length.saturating_sub(TAIL)))?;
    let mut tail = Vec::new();
    file.read_to_end(&mut tail)?;
    let text = String::from_utf8_lossy(&tail);
    Ok(text
        .lines()
        .rev()
        .filter(|line| line.contains("\"rate_limits\""))
        .find_map(|line| {
            let line = serde_json::from_str::<Line>(line).ok()?;
            let at = line.timestamp.as_deref().and_then(|at| {
                let at = chrono::DateTime::parse_from_rfc3339(at).ok()?;
                Some(
                    SystemTime::UNIX_EPOCH
                        + Duration::from_secs(u64::try_from(at.timestamp()).ok()?),
                )
            });
            Some((line.payload.rate_limits?, at))
        }))
}

fn usage(limits: Limits) -> Usage {
    let limit = |window_: Window| Limit {
        label: window_.window_minutes.map_or("额度".into(), window),
        percent: window_.used_percent,
        resets: window_
            .resets_at
            .map(|at| SystemTime::UNIX_EPOCH + Duration::from_secs(at)),
    };
    Usage {
        plan: limits.plan_type,
        limits: [limits.primary, limits.secondary]
            .into_iter()
            .flatten()
            .map(limit)
            .collect(),
        credits: limits.credits.and_then(credits),
        note: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_turn_s_limits_are_read_from_the_log_s_end() {
        let folder =
            std::env::temp_dir().join(format!("ely-tea-party-codex-{}", std::process::id()));
        let day = folder.join("sessions/2026/10/18");
        std::fs::create_dir_all(&day).expect("a temp folder");
        let turn = |percent: f32| {
            format!(
                r#"{{"timestamp":"2026-10-18T07:09:44.412Z","type":"event_msg","payload":{{"type":"token_count","rate_limits":{{"primary":{{"used_percent":{percent},"window_minutes":10080,"resets_at":1792875561}},"secondary":null,"credits":{{"has_credits":true,"unlimited":false,"balance":"5111.5139"}},"plan_type":"promax"}}}}}}"#
            )
        };
        let log = [
            turn(3.0),
            r#"{"type":"other","payload":{}}"#.into(),
            turn(4.0),
        ]
        .join("\n");
        std::fs::write(day.join("rollout-a.jsonl"), log + "\n").expect("a log");
        let (usage, at) = read(&folder).expect("read");
        std::fs::remove_dir_all(&folder).expect("the temp folder goes");
        assert_eq!(usage.plan.as_deref(), Some("promax"));
        assert_eq!(usage.credits.as_deref(), Some("5111.5"));
        assert_eq!(usage.limits.len(), 1);
        assert_eq!(usage.limits[0].label, "本周");
        assert_eq!(usage.limits[0].percent, 4.0, "the last turn's");
        let at = at.expect("its time").duration_since(SystemTime::UNIX_EPOCH);
        assert_eq!(at.expect("after the epoch").as_secs(), 1_792_307_384);
    }
}
