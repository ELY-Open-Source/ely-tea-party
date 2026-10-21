//! What a session's hooked agents did, in order: each prompt, tool, question and end, as the Activity tab lists them by round.

use std::{
    collections::{HashSet, VecDeque},
    time::{Duration, Instant},
};

use super::activity::Doing;

/// The most steps a session keeps; older ones drop.
pub const KEPT: usize = 200;

/// One step of an agent's work.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// The user's prompt, when the hook says it.
    Prompt(Option<String>),
    Tool {
        doing: Doing,
        name: String,
        detail: Option<String>,
        /// How long it ran, once its result came back.
        took: Option<Duration>,
        failed: bool,
    },
    Asked(Option<String>),
    Ended {
        failed: bool,
        took: Option<Duration>,
        /// The agent's last words, when the hook says them.
        said: Option<String>,
    },
}

/// A step and when it began.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub at: Instant,
    pub step: Step,
}

/// A session's steps, which outlive its agent until the session closes.
#[derive(Default)]
pub struct Timeline {
    pub entries: VecDeque<Entry>,
    /// The rounds, by start, the user opened or closed against their default.
    pub flipped: HashSet<Instant>,
}

/// One round: a prompt, the steps it took, and how it ended.
#[derive(Debug, PartialEq)]
pub struct Round<'a> {
    pub at: Instant,
    /// The prompt, when the round began with one; `Some(None)` when its text went unsaid.
    pub prompt: Option<Option<&'a str>>,
    pub steps: Vec<&'a Entry>,
    /// Whether it failed, how long it took and what the agent said last.
    pub end: Option<(bool, Option<Duration>, Option<&'a str>)>,
}

impl Timeline {
    /// Writes hook `event` in, a round having `started` then.
    pub fn record(
        &mut self,
        event: &str,
        (tool, detail): (Option<&str>, Option<&str>),
        started: Option<Instant>,
        now: Instant,
    ) {
        let detail = detail.map(str::to_string);
        let step = match event {
            "UserPromptSubmit" => Step::Prompt(detail),
            "PreToolUse" => {
                let name = tool.unwrap_or_default().to_string();
                Step::Tool {
                    doing: Doing::of(&name),
                    name,
                    detail,
                    took: None,
                    failed: false,
                }
            }
            "PostToolUse" | "PostToolUseFailure" => {
                let open = self
                    .entries
                    .iter_mut()
                    .rev()
                    .find_map(|entry| match &mut entry.step {
                        Step::Tool {
                            took: took @ None,
                            failed,
                            ..
                        } => Some((entry.at, took, failed)),
                        _ => None,
                    });
                if let Some((at, took, failed)) = open {
                    *took = Some(now.saturating_duration_since(at));
                    *failed = event == "PostToolUseFailure";
                }
                return;
            }
            "Notification" | "PermissionRequest" => Step::Asked(detail),
            "Stop" | "StopFailure" | "agent-turn-complete" => Step::Ended {
                failed: event == "StopFailure",
                took: started.map(|started| now.saturating_duration_since(started)),
                said: detail,
            },
            _ => return,
        };
        if self.entries.len() == KEPT {
            self.entries.pop_front();
        }
        self.entries.push_back(Entry { at: now, step });
    }

    /// The rounds, oldest first: each prompt starts one, as do steps before any prompt.
    pub fn rounds(&self) -> Vec<Round<'_>> {
        let mut rounds: Vec<Round> = Vec::new();
        for entry in &self.entries {
            if let Step::Prompt(prompt) = &entry.step {
                rounds.push(Round {
                    at: entry.at,
                    prompt: Some(prompt.as_deref()),
                    steps: Vec::new(),
                    end: None,
                });
                continue;
            }
            if rounds.is_empty() {
                rounds.push(Round {
                    at: entry.at,
                    prompt: None,
                    steps: Vec::new(),
                    end: None,
                });
            }
            let round = rounds.last_mut().expect("a round to write in");
            match &entry.step {
                Step::Ended { failed, took, said } => {
                    round.end = Some((*failed, *took, said.as_deref()))
                }
                _ => round.steps.push(entry),
            }
        }
        rounds
    }

    /// Opens a closed round, or closes an open one.
    pub fn flip(&mut self, round: Instant) {
        if !self.flipped.remove(&round) {
            self.flipped.insert(round);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_keep_their_order_and_tools_their_time() {
        let start = Instant::now();
        let mut log = Timeline::default();
        let second = Duration::from_secs(1);
        log.record("UserPromptSubmit", (None, Some("fix it")), None, start);
        log.record(
            "PreToolUse",
            (Some("Bash"), Some("cargo test")),
            None,
            start,
        );
        let failed = start + second * 3;
        log.record("PostToolUseFailure", (Some("Bash"), None), None, failed);
        log.record(
            "Stop",
            (None, Some("done")),
            Some(start),
            start + second * 5,
        );
        let steps: Vec<&Step> = log.entries.iter().map(|entry| &entry.step).collect();
        assert_eq!(steps[0], &Step::Prompt(Some("fix it".into())));
        assert!(matches!(
            steps[1],
            Step::Tool { took: Some(took), failed: true, .. } if *took == second * 3
        ));
        assert_eq!(
            steps[2],
            &Step::Ended {
                failed: false,
                took: Some(second * 5),
                said: Some("done".into()),
            }
        );
        for _ in 0..KEPT {
            log.record("UserPromptSubmit", (None, None), None, start);
        }
        assert_eq!(log.entries.len(), KEPT, "old steps drop");
    }

    #[test]
    fn prompts_start_rounds_and_ends_close_them() {
        let start = Instant::now();
        let at = |n: u64| start + Duration::from_secs(n);
        let mut log = Timeline::default();
        log.record("PreToolUse", (Some("Read"), Some("a.rs")), None, at(0));
        log.record("UserPromptSubmit", (None, Some("one")), None, at(1));
        log.record("PreToolUse", (Some("Edit"), Some("b.rs")), None, at(2));
        log.record("Stop", (None, Some("did one")), Some(at(1)), at(4));
        log.record("UserPromptSubmit", (None, None), None, at(5));
        log.record("Notification", (None, Some("may I?")), None, at(6));
        let rounds = log.rounds();
        assert_eq!(rounds.len(), 3);
        assert_eq!((rounds[0].prompt, rounds[0].steps.len()), (None, 1));
        assert_eq!(rounds[1].prompt, Some(Some("one")));
        assert_eq!(rounds[1].steps.len(), 1);
        let ended = (false, Some(Duration::from_secs(3)), Some("did one"));
        assert_eq!(rounds[1].end, Some(ended));
        assert_eq!((rounds[2].prompt, rounds[2].end), (Some(None), None));
        assert!(matches!(rounds[2].steps[0].step, Step::Asked(Some(_))));
    }

    #[test]
    fn a_flip_opens_or_closes_a_round_and_a_second_undoes_it() {
        let at = Instant::now();
        let mut log = Timeline::default();
        log.flip(at);
        assert!(log.flipped.contains(&at));
        log.flip(at);
        assert!(log.flipped.is_empty());
    }
}
