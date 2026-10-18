//! Codex writes its plan's limits into each turn's `token_count` event in its session logs, so they are read there, with no login.

use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use anyhow::Context as _;
use serde::Deserialize;

use super::{Limit, Usage, window};

/// How much of a log's end is searched for its last limits; a log runs to tens of megabytes.
const TAIL: u64 = 2 << 20;

/// How many of the newest logs are tried, since a log just begun holds no turn yet.
const LOGS: usize = 8;

#[derive(Deserialize)]
struct Line {
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

#[derive(Deserialize)]
struct Credits {
    has_credits: bool,
    unlimited: bool,
    balance: Option<String>,
}

pub fn read() -> anyhow::Result<Usage> {
    let home = match std::env::var_os("CODEX_HOME") {
        Some(home) => PathBuf::from(home),
        None => dirs::home_dir()
            .context("usage: no home folder")?
            .join(".codex"),
    };
    let logs = newest(&home.join("sessions"))?;
    for log in &logs {
        if let Some(limits) = last_limits(log)? {
            return Ok(usage(limits));
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

/// The limits `log`'s last turn reported, from its end.
fn last_limits(log: &Path) -> anyhow::Result<Option<Limits>> {
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
        .find_map(|line| serde_json::from_str::<Line>(line).ok()?.payload.rate_limits))
}

fn usage(limits: Limits) -> Usage {
    let limit = |window_: Window| Limit {
        label: window_.window_minutes.map_or("额度".into(), window),
        percent: window_.used_percent,
        resets: window_
            .resets_at
            .map(|at| SystemTime::UNIX_EPOCH + Duration::from_secs(at)),
    };
    let credits = limits.credits.and_then(|credits| match credits {
        Credits {
            unlimited: true, ..
        } => Some("不限".into()),
        Credits {
            has_credits: true,
            balance: Some(balance),
            ..
        } => Some(
            balance
                .parse::<f64>()
                .map_or(balance, |b| format!("{b:.1}")),
        ),
        _ => None,
    });
    Usage {
        plan: limits.plan_type,
        limits: [limits.primary, limits.secondary]
            .into_iter()
            .flatten()
            .map(limit)
            .collect(),
        credits,
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
                r#"{{"type":"event_msg","payload":{{"type":"token_count","rate_limits":{{"primary":{{"used_percent":{percent},"window_minutes":10080,"resets_at":1792875561}},"secondary":null,"credits":{{"has_credits":true,"unlimited":false,"balance":"5111.5139"}},"plan_type":"promax"}}}}}}"#
            )
        };
        let log = [
            turn(3.0),
            r#"{"type":"other","payload":{}}"#.into(),
            turn(4.0),
        ]
        .join("\n");
        std::fs::write(day.join("rollout-a.jsonl"), log + "\n").expect("a log");
        let logs = newest(&folder.join("sessions")).expect("logs");
        let limits = last_limits(&logs[0]).expect("read").expect("limits");
        std::fs::remove_dir_all(&folder).expect("the temp folder goes");
        let usage = usage(limits);
        assert_eq!(usage.plan.as_deref(), Some("promax"));
        assert_eq!(usage.credits.as_deref(), Some("5111.5"));
        assert_eq!(usage.limits.len(), 1);
        assert_eq!(usage.limits[0].label, "本周");
        assert_eq!(usage.limits[0].percent, 4.0, "the last turn's");
    }
}
