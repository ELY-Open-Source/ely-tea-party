//! How much of an agent's subscription is used, from the usage endpoint each agent asks itself.

use std::time::{Duration, SystemTime};

use crate::agents::Kind;

mod antigravity;
mod claude;
mod codex;

/// The subscription an agent runs on, for the agents whose use can be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Plan {
    Claude,
    Codex,
    Antigravity,
}

impl Plan {
    /// The plan of an agent of `kind` run as `program`, which tells Antigravity from Gemini's own CLI: they share a character.
    pub fn of(kind: Kind, program: Option<&str>) -> Option<Plan> {
        match (kind, program) {
            (Kind::Claude, _) => Some(Plan::Claude),
            (Kind::Gpt, _) => Some(Plan::Codex),
            (Kind::Gemini, Some("agy" | "antigravity")) => Some(Plan::Antigravity),
            _ => None,
        }
    }

    /// How long a reading holds: Claude's endpoint is asked rarely, Codex's more often.
    pub fn fresh(self) -> Duration {
        match self {
            Plan::Claude => Duration::from_secs(300),
            Plan::Codex => Duration::from_secs(120),
            Plan::Antigravity => Duration::from_secs(180),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Plan::Claude => "Claude",
            Plan::Codex => "Codex",
            Plan::Antigravity => "Antigravity",
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
    /// When each banked reset still to be used expires, for those that do.
    pub banked: Vec<SystemTime>,
}

/// One window of use: how much of it is spent and when it starts again.
#[derive(Clone, Debug, PartialEq)]
pub struct Limit {
    pub label: String,
    pub percent: f32,
    pub resets: Option<SystemTime>,
}

/// How near a banked reset's expiry is, as the sessions column's foot colors it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nearness {
    /// Within [`SOON`].
    Soon,
    /// Within [`VERY_SOON`].
    VerySoon,
}

/// A banked reset expiring this near is noted.
const SOON: Duration = Duration::from_secs(3 * 86400);
/// A banked reset expiring this near is noted in red.
const VERY_SOON: Duration = Duration::from_secs(86400);

/// How `usage`'s banked resets stand at `now`, if any expires within [`SOON`]: how near the soonest is, how many do, and when the soonest ends.
pub fn expiring_banked(usage: &Usage, now: SystemTime) -> Option<(Nearness, usize, SystemTime)> {
    let left = |expires: &&SystemTime| expires.duration_since(now).is_ok_and(|left| left <= SOON);
    let expiring: Vec<SystemTime> = usage.banked.iter().filter(left).copied().collect();
    let soonest = expiring.iter().min().copied()?;
    let near = match soonest.duration_since(now).ok()? <= VERY_SOON {
        true => Nearness::VerySoon,
        false => Nearness::Soon,
    };
    Some((near, expiring.len(), soonest))
}

/// Reads `plan`'s usage; it blocks, so it runs off the main thread.
pub fn read(plan: Plan) -> anyhow::Result<Usage> {
    match plan {
        Plan::Claude => claude::read(),
        Plan::Codex => codex::read(),
        Plan::Antigravity => antigravity::read(),
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
    format!("{}后重置", time_left(resets, now))
}

/// How long until `at`, counted from `now`: `2 天 5 小时`.
pub fn time_left(at: SystemTime, now: SystemTime) -> String {
    let left = at.duration_since(now).unwrap_or_default().as_secs();
    let (days, hours, minutes) = (left / 86400, left % 86400 / 3600, left % 3600 / 60);
    match (days, hours) {
        (0, 0) => format!("{} 分钟", minutes.max(1)),
        (0, h) => format!("{h} 小时 {minutes} 分钟"),
        (d, h) => format!("{d} 天 {h} 小时"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_banked_reset_is_noted_within_three_days_and_marked_within_one() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let hours = |n: u64| now + Duration::from_secs(n * 3600);
        let near = |banked: Vec<SystemTime>| {
            let usage = Usage {
                plan: None,
                limits: Vec::new(),
                credits: None,
                note: None,
                banked,
            };
            expiring_banked(&usage, now)
        };
        assert_eq!(near(vec![]), None, "none banked");
        assert_eq!(near(vec![hours(73)]), None, "over three days");
        assert_eq!(near(vec![hours(72)]), Some((Nearness::Soon, 1, hours(72))));
        assert_eq!(near(vec![hours(25)]), Some((Nearness::Soon, 1, hours(25))));
        assert_eq!(
            near(vec![hours(24)]),
            Some((Nearness::VerySoon, 1, hours(24)))
        );
        assert_eq!(
            near(vec![now - Duration::from_secs(60)]),
            None,
            "already gone"
        );
        assert_eq!(
            near(vec![hours(200), hours(60), hours(10)]),
            Some((Nearness::VerySoon, 2, hours(10))),
            "only those near count"
        );
    }

    #[test]
    fn only_antigravity_of_gemini_s_programs_has_a_plan_to_read() {
        assert_eq!(Plan::of(Kind::Gemini, Some("agy")), Some(Plan::Antigravity));
        assert_eq!(Plan::of(Kind::Gemini, Some("gemini")), None);
        assert_eq!(Plan::of(Kind::Gemini, None), None);
        assert_eq!(Plan::of(Kind::Claude, None), Some(Plan::Claude));
    }

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
