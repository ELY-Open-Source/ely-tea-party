//! How much of an agent's subscription is used, from the usage endpoint each agent asks itself.

use std::time::{Duration, SystemTime};

use crate::agents::Kind;

mod claude;
mod codex;

/// The subscription an agent runs on, for the agents whose use can be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Plan {
    Claude,
    Codex,
}

impl Plan {
    pub fn of(kind: Kind) -> Option<Plan> {
        match kind {
            Kind::Claude => Some(Plan::Claude),
            Kind::Gpt => Some(Plan::Codex),
            _ => None,
        }
    }

    /// How long a reading holds: Claude's endpoint is asked rarely, Codex's more often.
    pub fn fresh(self) -> Duration {
        match self {
            Plan::Claude => Duration::from_secs(300),
            Plan::Codex => Duration::from_secs(120),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Plan::Claude => "Claude",
            Plan::Codex => "Codex",
        }
    }
}

/// A subscription's state as its service gives it.
#[derive(Clone, Debug, PartialEq)]
pub struct Usage {
    pub plan: Option<String>,
    pub limits: Vec<Limit>,
    /// Credits left, on plans that have them.
    pub credits: Option<String>,
    /// Where an older reading came from, while the endpoint cannot be asked.
    pub note: Option<String>,
}

/// One window of use: how much of it is spent and when it starts again.
#[derive(Clone, Debug, PartialEq)]
pub struct Limit {
    pub label: String,
    pub percent: f32,
    pub resets: Option<SystemTime>,
}

/// Reads `plan`'s usage; it blocks, so it runs off the main thread.
pub fn read(plan: Plan) -> anyhow::Result<Usage> {
    match plan {
        Plan::Claude => claude::read(),
        Plan::Codex => codex::read(),
    }
}

/// A window of `minutes` as a limit names it.
fn window(minutes: u64) -> String {
    match minutes {
        300 => "5 小时".into(),
        10080 => "本周".into(),
        m if m % 1440 == 0 => format!("{} 天", m / 1440),
        m if m % 60 == 0 => format!("{} 小时", m / 60),
        m => format!("{m} 分钟"),
    }
}

/// When `resets` comes, counted from `now`: `2 天 5 小时后重置`.
pub fn resets_in(resets: SystemTime, now: SystemTime) -> String {
    let left = resets.duration_since(now).unwrap_or_default().as_secs();
    let (days, hours, minutes) = (left / 86400, left % 86400 / 3600, left % 3600 / 60);
    let when = match (days, hours) {
        (0, 0) => format!("{} 分钟", minutes.max(1)),
        (0, h) => format!("{h} 小时 {minutes} 分钟"),
        (d, h) => format!("{d} 天 {h} 小时"),
    };
    format!("{when}后重置")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_and_resets_read_as_people_say_them() {
        assert_eq!(window(300), "5 小时");
        assert_eq!(window(10080), "本周");
        assert_eq!(window(2880), "2 天");
        assert_eq!(window(90), "90 分钟");
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let after = |secs| resets_in(now + Duration::from_secs(secs), now);
        assert_eq!(after(2 * 86400 + 5 * 3600 + 59), "2 天 5 小时后重置");
        assert_eq!(after(3 * 3600 + 120), "3 小时 2 分钟后重置");
        assert_eq!(after(10), "1 分钟后重置");
        assert_eq!(resets_in(now, now + Duration::from_secs(5)), "1 分钟后重置");
    }
}
