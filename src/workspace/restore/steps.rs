//! What each session's agents did outlives the app, in a file beside the saved sessions.

use std::{
    collections::HashMap,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    agents::timeline::{Entry, KEPT, Step, Timeline},
    sessions::{LinkId, Session},
};

/// The least time between two saves.
const EVERY: Duration = Duration::from_secs(5);

/// A step as saved: when it began, in milliseconds of the clock on the wall.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct KeptStep {
    at: u64,
    step: Step,
}

/// Each session's steps, by the link that outlives it.
pub type Kept = HashMap<LinkId, Vec<KeptStep>>;

/// When the steps were last saved, and how many changes that took in.
#[derive(Default)]
pub struct Keeper {
    changes: u64,
    at: Option<Instant>,
}

impl Keeper {
    /// Saves `sessions`' steps when they changed, at most once in [`EVERY`].
    pub fn keep(&mut self, sessions: &[Session], now: Instant) {
        let changes = sessions.iter().map(|session| session.log.changes).sum();
        let soon = self
            .at
            .is_some_and(|at| now.saturating_duration_since(at) < EVERY);
        if changes == self.changes || soon {
            return;
        }
        (self.changes, self.at) = (changes, Some(now));
        let wall = millis(SystemTime::now());
        let kept: Kept = (sessions.iter())
            .filter(|session| !session.log.entries.is_empty())
            .map(|session| (session.link_id, saved(&session.log, wall, now)))
            .collect();
        if let Err(error) = save(&kept) {
            log::error!("restore: the steps stay unsaved: {error:#}");
        }
    }
}

fn save(kept: &Kept) -> anyhow::Result<()> {
    super::replace(&super::file("activity")?, serde_json::to_string(kept)?)
}

/// The steps saved last; a file that does not parse is logged and passed by.
pub fn load() -> Kept {
    let read = || -> anyhow::Result<Kept> {
        match std::fs::read_to_string(super::file("activity")?) {
            Ok(text) => Ok(serde_json::from_str(&text)?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Kept::new()),
            Err(error) => Err(error.into()),
        }
    };
    read().unwrap_or_else(|error| {
        log::error!("restore: the steps saved last are passed by: {error:#}");
        Kept::new()
    })
}

/// Session `link`'s steps from `kept`, as a log that goes on from now.
pub fn take(kept: &mut Kept, link: LinkId) -> Option<Timeline> {
    let steps = kept.remove(&link)?;
    Some(restored(steps, millis(SystemTime::now()), Instant::now()))
}

fn millis(time: SystemTime) -> u64 {
    let since = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    since.as_millis() as u64
}

/// `log`'s steps, `wall` being what the clock on the wall reads at `now`.
fn saved(log: &Timeline, wall: u64, now: Instant) -> Vec<KeptStep> {
    let keep = |entry: &Entry| KeptStep {
        at: wall.saturating_sub(now.saturating_duration_since(entry.at).as_millis() as u64),
        step: entry.step.clone(),
    };
    log.entries.iter().map(keep).collect()
}

/// A log of `steps`, oldest first. An `Instant` reaches back no further than the machine's start, so older steps sit there, in order, each its own moment, since a round's start names it.
fn restored(steps: Vec<KeptStep>, wall: u64, now: Instant) -> Timeline {
    let skip = steps.len().saturating_sub(KEPT);
    let mut log = Timeline::default();
    let mut last: Option<Instant> = None;
    for KeptStep { at, step } in steps.into_iter().skip(skip) {
        let mut ago = Duration::from_millis(wall.saturating_sub(at));
        let mut when = now.checked_sub(ago);
        while when.is_none() {
            ago /= 2;
            when = now.checked_sub(ago);
        }
        let mut when = when.unwrap_or(now);
        if let Some(last) = last.filter(|last| when <= *last) {
            when = last + Duration::from_nanos(1);
        }
        last = Some(when);
        log.entries.push_back(Entry { at: when, step });
    }
    log
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::{activity::Doing, timeline::Timeline};

    fn logged(now: Instant) -> Timeline {
        let mut log = Timeline::default();
        let at = |ago: u64| now - Duration::from_secs(ago);
        log.record("UserPromptSubmit", (None, Some("p"), None), None, at(30));
        log.record(
            "PreToolUse",
            (Some("Read"), Some("a"), Some("1")),
            None,
            at(20),
        );
        log.record(
            "PostToolUse",
            (Some("Read"), Some("a"), Some("1")),
            None,
            at(10),
        );
        log.record("Stop", (None, Some("done"), None), Some(at(30)), at(5));
        log
    }

    #[test]
    fn steps_come_back_as_they_were_and_as_old() {
        let now = Instant::now();
        let log = logged(now);
        let wall = 1_800_000_000_000;
        let kept: Kept = HashMap::from([(LinkId::default(), saved(&log, wall, now))]);
        let text = serde_json::to_string(&kept).expect("written");
        let read: Kept = serde_json::from_str(&text).expect("read");
        let (_, steps) = read.into_iter().next().expect("the session");
        let later = now + Duration::from_secs(60);
        let back = restored(steps, wall + 60_000, later);
        let steps = |log: &Timeline| {
            log.entries
                .iter()
                .map(|e| e.step.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(steps(&back), steps(&log));
        let tool = &back.entries[1];
        assert!(matches!(
            &tool.step,
            Step::Tool { doing: Doing::Reading, took: Some(took), .. } if *took == Duration::from_secs(10)
        ));
        assert_eq!(later.duration_since(tool.at), Duration::from_secs(80));
        assert_eq!(back.rounds().len(), 1);
    }

    #[test]
    fn steps_older_than_the_machine_s_clock_keep_their_order_and_their_own_moments() {
        let now = Instant::now();
        let step = || Step::Asked(None);
        let centuries = 1 << 60;
        let steps = vec![
            KeptStep {
                at: 0,
                step: step(),
            },
            KeptStep {
                at: 1,
                step: step(),
            },
            KeptStep {
                at: centuries,
                step: step(),
            },
        ];
        let log = restored(steps, centuries + 1, now);
        let at: Vec<Instant> = log.entries.iter().map(|entry| entry.at).collect();
        assert!(at[0] < at[1] && at[1] < at[2]);
    }

    #[test]
    fn a_file_longer_than_a_session_keeps_only_the_newest_steps() {
        let now = Instant::now();
        let steps = (0..KEPT as u64 + 5)
            .map(|at| KeptStep {
                at,
                step: Step::Prompt(Some(at.to_string())),
            })
            .collect();
        let log = restored(steps, 10_000, now);
        assert_eq!(log.entries.len(), KEPT);
        assert_eq!(log.entries[0].step, Step::Prompt(Some("5".into())));
    }
}
