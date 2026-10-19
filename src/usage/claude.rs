//! Claude Code's plan limits, from the usage endpoint its `/usage` asks, with the login Claude Code keeps; that login is never refreshed here, which would sign Claude Code out.

use std::time::{Duration, SystemTime};

#[cfg(target_os = "macos")]
use std::process::Command;

use anyhow::Context as _;
use serde::Deserialize;

use super::{Limit, Usage};

const ENDPOINT: &str = "https://api.anthropic.com/api/oauth/usage";

#[derive(Deserialize)]
struct Stored {
    #[serde(rename = "claudeAiOauth")]
    oauth: Login,
}

#[derive(Deserialize)]
struct Login {
    #[serde(rename = "accessToken")]
    token: String,
    /// When the token ends, in milliseconds since the epoch.
    #[serde(rename = "expiresAt")]
    expires: Option<u64>,
    #[serde(rename = "subscriptionType")]
    subscription: Option<String>,
}

#[derive(Deserialize)]
struct Answer {
    #[serde(default)]
    limits: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    kind: String,
    percent: f32,
    resets_at: Option<String>,
    scope: Option<Scope>,
}

#[derive(Deserialize)]
struct Scope {
    model: Option<Model>,
}

#[derive(Deserialize)]
struct Model {
    display_name: Option<String>,
}

pub fn read() -> anyhow::Result<Usage> {
    let login = login()?;
    if login.expires.is_some_and(|expires| {
        SystemTime::UNIX_EPOCH + Duration::from_millis(expires) <= SystemTime::now()
    }) {
        anyhow::bail!(
            "usage: Claude Code's login has expired; it renews when Claude Code next runs"
        );
    }
    let headers = format!(
        "Authorization: Bearer {}\nanthropic-beta: oauth-2025-04-20\nUser-Agent: ely-tea-party\n",
        login.token
    );
    let answer: Answer = serde_json::from_slice(&crate::curl::get("usage", ENDPOINT, &headers)?)
        .context("usage: Claude's answer does not parse")?;
    Ok(usage(answer, login.subscription))
}

/// The login Claude Code keeps: in the keychain on macOS, else in its credentials file.
fn login() -> anyhow::Result<Login> {
    #[cfg(target_os = "macos")]
    {
        let found = Command::new("security")
            .args([
                "find-generic-password",
                "-s",
                "Claude Code-credentials",
                "-w",
            ])
            .output()
            .context("usage: cannot run security")?;
        if found.status.success() {
            return serde_json::from_slice::<Stored>(&found.stdout)
                .map(|stored| stored.oauth)
                .context("usage: Claude Code's keychain item does not parse");
        }
    }
    let file = dirs::home_dir()
        .context("usage: no home folder")?
        .join(".claude")
        .join(".credentials.json");
    let text = std::fs::read(&file).context("usage: Claude Code is not logged in")?;
    serde_json::from_slice::<Stored>(&text)
        .map(|stored| stored.oauth)
        .with_context(|| format!("usage: {} does not parse", file.display()))
}

fn usage(answer: Answer, plan: Option<String>) -> Usage {
    let limit = |entry: Entry| {
        let label = match entry.kind.as_str() {
            "session" => "5 小时".to_string(),
            "weekly_all" => "本周".to_string(),
            kind => {
                let model = entry.scope.as_ref().and_then(|scope| scope.model.as_ref());
                let named = model.and_then(|model| model.display_name.clone());
                named.map_or_else(|| kind.to_string(), |name| format!("本周 · {name}"))
            }
        };
        let resets = entry.resets_at.as_deref().and_then(|at| {
            let at = chrono::DateTime::parse_from_rfc3339(at).ok()?;
            Some(SystemTime::UNIX_EPOCH + Duration::from_secs(u64::try_from(at.timestamp()).ok()?))
        });
        Limit {
            label,
            percent: entry.percent,
            resets,
        }
    };
    Usage {
        plan,
        limits: answer.limits.into_iter().map(limit).collect(),
        credits: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_limit_reads_with_its_window_and_reset() {
        let answer: Answer = serde_json::from_str(
            r#"{"five_hour":{"utilization":0.0},"limits":[
                {"kind":"session","group":"session","percent":0,"severity":"normal","resets_at":null,"scope":null,"is_active":false},
                {"kind":"weekly_all","group":"weekly","percent":31,"severity":"normal","resets_at":"2026-10-20T22:00:00.450457+00:00","scope":null,"is_active":true},
                {"kind":"weekly_scoped","group":"weekly","percent":0,"severity":"normal","resets_at":"2026-10-20T22:00:00+00:00","scope":{"model":{"id":null,"display_name":"Fable"},"surface":null},"is_active":false}
            ]}"#,
        )
        .expect("parses");
        let usage = usage(answer, Some("max".into()));
        let labels: Vec<&str> = usage
            .limits
            .iter()
            .map(|limit| limit.label.as_str())
            .collect();
        assert_eq!(labels, ["5 小时", "本周", "本周 · Fable"]);
        assert_eq!(usage.limits[1].percent, 31.0);
        assert_eq!(usage.limits[0].resets, None);
        let reset = usage.limits[1].resets.expect("a reset");
        let secs = reset
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("after the epoch")
            .as_secs();
        assert_eq!(secs, 1_792_533_600);
    }
}
