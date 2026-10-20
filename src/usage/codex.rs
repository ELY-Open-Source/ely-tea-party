//! Codex's plan limits, from the usage endpoint Codex itself asks, with the ChatGPT login Codex keeps, which is never refreshed here, as that would sign Codex out; while the endpoint cannot be asked, from Codex's session logs, marked with their age.

mod logs;

use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use anyhow::Context as _;
use base64::Engine as _;
use serde::Deserialize;

use super::{Limit, Usage, window};

const ENDPOINT: &str = "https://chatgpt.com/backend-api/wham/usage";

#[derive(Deserialize)]
struct Auth {
    tokens: Option<Tokens>,
}

#[derive(Deserialize)]
struct Tokens {
    access_token: String,
    account_id: Option<String>,
}

/// The claim of the access token the check before asking reads.
#[derive(Deserialize)]
struct Claims {
    exp: u64,
}

#[derive(Deserialize)]
struct Answer {
    plan_type: Option<String>,
    rate_limit: Option<RateLimit>,
    credits: Option<Credits>,
}

#[derive(Deserialize)]
struct RateLimit {
    primary_window: Option<Window>,
    secondary_window: Option<Window>,
}

#[derive(Deserialize)]
struct Window {
    used_percent: f32,
    limit_window_seconds: Option<u64>,
    reset_at: Option<u64>,
}

/// Credits as the endpoint and the logs both give them; the endpoint adds how many messages they buy, a range.
#[derive(Deserialize)]
struct Credits {
    has_credits: bool,
    unlimited: bool,
    balance: Option<String>,
    #[serde(default)]
    approx_local_messages: Option<[u64; 2]>,
}

pub fn read() -> anyhow::Result<Usage> {
    let home = match std::env::var_os("CODEX_HOME") {
        Some(home) => PathBuf::from(home),
        None => dirs::home_dir()
            .context("usage: no home folder")?
            .join(".codex"),
    };
    let asked = match ask(&home) {
        Ok(usage) => return Ok(usage),
        Err(error) => error,
    };
    log::info!("{asked:#}; Codex's logs are read instead");
    let (mut usage, at) =
        logs::read(&home).map_err(|logged| anyhow::anyhow!("{asked:#}; {logged:#}"))?;
    let when = at.map_or("最后一次".into(), |at| ago(at, SystemTime::now()));
    usage.note = Some(format!("接口没读到，这是本机 Codex 日志{when}的记录"));
    Ok(usage)
}

/// The endpoint's answer, with the login in `home`'s `auth.json`.
fn ask(home: &Path) -> anyhow::Result<Usage> {
    let file = home.join("auth.json");
    let text = std::fs::read(&file).context("usage: Codex is not logged in")?;
    let auth: Auth = serde_json::from_slice(&text)
        .with_context(|| format!("usage: {} does not parse", file.display()))?;
    let tokens = auth
        .tokens
        .context("usage: Codex is logged in with an API key, which has no plan")?;
    if expired(&tokens.access_token, SystemTime::now()) {
        anyhow::bail!("usage: Codex's login has expired; it renews when Codex next runs");
    }
    let mut headers = format!(
        "Authorization: Bearer {}\nUser-Agent: ely-tea-party\n",
        tokens.access_token
    );
    if let Some(account) = &tokens.account_id {
        headers.push_str(&format!("ChatGPT-Account-Id: {account}\n"));
    }
    let answer: Answer = serde_json::from_slice(&crate::curl::get("usage", ENDPOINT, &headers)?)
        .context("usage: Codex's answer does not parse")?;
    Ok(usage(answer))
}

/// Whether the access token's expiry has passed at `now`; a token that does not read is left to the endpoint to judge.
fn expired(token: &str, now: SystemTime) -> bool {
    let claims = token.split('.').nth(1).and_then(|part| {
        let part = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(part.trim_end_matches('='))
            .ok()?;
        serde_json::from_slice::<Claims>(&part).ok()
    });
    claims.is_some_and(|claims| SystemTime::UNIX_EPOCH + Duration::from_secs(claims.exp) <= now)
}

fn usage(answer: Answer) -> Usage {
    let limit = |window_: Window| Limit {
        label: window_
            .limit_window_seconds
            .map_or("额度".into(), |seconds| window(seconds / 60)),
        percent: window_.used_percent,
        resets: window_
            .reset_at
            .map(|at| SystemTime::UNIX_EPOCH + Duration::from_secs(at)),
    };
    let windows = answer
        .rate_limit
        .map(|limits| [limits.primary_window, limits.secondary_window]);
    Usage {
        plan: answer.plan_type,
        limits: windows.into_iter().flatten().flatten().map(limit).collect(),
        credits: answer.credits.and_then(credits),
        note: None,
    }
}

/// Credits as the foot shows them: unlimited, or the balance and the messages it buys, if any.
fn credits(credits: Credits) -> Option<String> {
    match credits {
        Credits {
            unlimited: true, ..
        } => Some("不限".into()),
        Credits {
            has_credits: true,
            balance: Some(balance),
            approx_local_messages,
            ..
        } => {
            let balance = balance
                .parse::<f64>()
                .map_or(balance, |b| format!("{b:.1}"));
            Some(match approx_local_messages {
                Some([least, most]) => format!("{balance} · 约 {least}–{most} 条消息"),
                None => balance,
            })
        }
        _ => None,
    }
}

/// How long before `now` `at` was: `2 天前`.
fn ago(at: SystemTime, now: SystemTime) -> String {
    let secs = now.duration_since(at).unwrap_or_default().as_secs();
    match (secs / 86400, secs / 3600, secs / 60) {
        (0, 0, 0) => "刚才".into(),
        (0, 0, m) => format!(" {m} 分钟前"),
        (0, h, _) => format!(" {h} 小时前"),
        (d, _, _) => format!(" {d} 天前"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_endpoint_s_answer_reads_as_windows_credits_and_messages() {
        let answer: Answer = serde_json::from_str(
            r#"{"user_id":"user-x","plan_type":"promax","rate_limit":{"allowed":true,"limit_reached":false,
                "primary_window":{"used_percent":4,"limit_window_seconds":604800,"reset_after_seconds":397878,"reset_at":1792875561},
                "secondary_window":{"used_percent":12.5,"limit_window_seconds":18000,"reset_after_seconds":60,"reset_at":1792480000}},
                "code_review_rate_limit":null,"additional_rate_limits":null,
                "credits":{"has_credits":true,"unlimited":false,"overage_limit_reached":false,"balance":"5111.5139360000",
                "approx_local_messages":[1278,6645],"approx_cloud_messages":[204,1278]}}"#,
        )
        .expect("parses");
        let usage = usage(answer);
        assert_eq!(usage.plan.as_deref(), Some("promax"));
        let labels: Vec<&str> = usage.limits.iter().map(|l| l.label.as_str()).collect();
        assert_eq!(labels, ["本周", "5 小时"]);
        assert_eq!(usage.limits[0].percent, 4.0);
        let reset = usage.limits[0].resets.expect("a reset");
        let secs = reset.duration_since(SystemTime::UNIX_EPOCH).expect("later");
        assert_eq!(secs.as_secs(), 1_792_875_561);
        assert_eq!(
            usage.credits.as_deref(),
            Some("5111.5 · 约 1278–6645 条消息")
        );
        assert_eq!(usage.note, None);
        let bare: Answer =
            serde_json::from_str(r#"{"plan_type":"free","rate_limit":null,"credits":null}"#)
                .expect("parses");
        assert!(super::usage(bare).limits.is_empty());
    }

    #[test]
    fn a_token_past_its_expiry_is_not_sent() {
        let token = |exp: u64| {
            let claims = format!(r#"{{"exp":{exp},"scp":["openid"]}}"#);
            let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims);
            format!("header.{claims}.signature")
        };
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(2_000);
        assert!(expired(&token(1_999), now));
        assert!(!expired(&token(2_001), now));
        assert!(!expired("not-a-jwt", now), "the endpoint judges it");
    }

    #[test]
    fn a_reading_s_age_reads_as_people_say_it() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let before = |secs| ago(now - Duration::from_secs(secs), now);
        assert_eq!(before(10), "刚才");
        assert_eq!(before(5 * 60), " 5 分钟前");
        assert_eq!(before(3 * 3600 + 5), " 3 小时前");
        assert_eq!(before(2 * 86400 + 7), " 2 天前");
    }
}
