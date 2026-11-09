//! What a session's hooked agents did, in order: each prompt, tool, question and end, as the Activity tab lists them by round.

use std::{
    collections::{HashSet, VecDeque},
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use super::activity::Doing;

/// The most steps a session keeps; older ones drop.
pub const KEPT: usize = 200;

/// What a hook says of a step: its tool, what it touches, and the call's own id.
pub type Said<'a> = (Option<&'a str>, Option<&'a str>, Option<&'a str>);

/// One step of an agent's work.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
        /// The call's own id, which its result names too.
        #[serde(default)]
        id: Option<String>,
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
    /// Counts what was written in, so a save knows there is more.
    pub changes: u64,
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
    /// Writes hook `event` in, a round having `started` then; false for a result that answers no call.
    pub fn record(
        &mut self,
        event: &str,
        (tool, detail, id): Said<'_>,
        started: Option<Instant>,
        now: Instant,
    ) -> bool {
        self.changes += 1;
        let last = self.entries.back().map(|entry| &entry.step);
        let step = match event {
            "UserPromptSubmit" => Step::Prompt(detail.map(str::to_string)),
            "PreToolUse" => {
                let name = tool.unwrap_or_default().to_string();
                Step::Tool {
                    doing: Doing::of(&name),
                    name,
                    detail: detail.map(str::to_string),
                    took: None,
                    failed: false,
                    id: id.map(str::to_string),
                }
            }
            "PostToolUse" | "PostToolUseFailure" => {
                let Some(at) = self.call((tool, detail, id)) else {
                    return false;
                };
                let entry = &mut self.entries[at];
                let ran = now.saturating_duration_since(entry.at);
                if let Step::Tool { took, failed, .. } = &mut entry.step {
                    *took = Some(ran);
                    *failed = event == "PostToolUseFailure";
                }
                return true;
            }
            // A permission asked is announced twice: its request, then a notice.
            "Notification" if matches!(last, Some(Step::Asked(_))) => return true,
            "Notification" | "PermissionRequest" => Step::Asked(detail.map(str::to_string)),
            // Codex ends a turn by its hook and its notify.
            "Stop" | "agent-turn-complete"
                if started.is_none() && matches!(last, Some(Step::Ended { .. })) =>
            {
                return true;
            }
            "Stop" | "StopFailure" | "agent-turn-complete" => Step::Ended {
                failed: event == "StopFailure",
                took: started.map(|started| now.saturating_duration_since(started)),
                said: detail.map(str::to_string),
            },
            _ => return true,
        };
        if self.entries.len() == KEPT {
            self.entries.pop_front();
        }
        self.entries.push_back(Entry { at: now, step });
        true
    }

    /// The open call a result answers, as a place in the entries: by id, else the oldest of its tool and detail, else of its tool; a result naming no tool answers the newest.
    fn call(&self, (tool, detail, id): Said<'_>) -> Option<usize> {
        let open: Vec<(usize, &str, Option<&str>, Option<&str>)> =
            (self.entries.iter().enumerate())
                .filter_map(|(at, entry)| match &entry.step {
                    Step::Tool {
                        name,
                        detail,
                        took: None,
                        id,
                        ..
                    } => Some((at, name.as_str(), detail.as_deref(), id.as_deref())),
                    _ => None,
                })
                .collect();
        let Some(tool) = tool else {
            return open.last().map(|call| call.0);
        };
        let by_id = id.and_then(|id| open.iter().find(|call| call.3 == Some(id)));
        // A call with an id of its own answers only that id.
        let free = |call: &&(usize, &str, Option<&str>, Option<&str>)| {
            call.1 == tool && (id.is_none() || call.3.is_none())
        };
        by_id
            .or_else(|| open.iter().filter(free).find(|call| call.2 == detail))
            .or_else(|| open.iter().find(|call| free(call)))
            .map(|call| call.0)
    }

    /// Whether a step began at `at`, as a round's start names it.
    pub fn holds(&self, at: Instant) -> bool {
        self.entries.iter().any(|entry| entry.at == at)
    }

    /// The rounds, oldest first: each prompt starts one, as do steps before any prompt or after a round's end, which a turn no one typed takes.
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
            let ended = rounds.last().is_some_and(|round| round.end.is_some());
            if rounds.is_empty() || (ended && !matches!(entry.step, Step::Ended { .. })) {
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
        log.record(
            "UserPromptSubmit",
            (None, Some("fix it"), None),
            None,
            start,
        );
        log.record(
            "PreToolUse",
            (Some("Bash"), Some("cargo test"), None),
            None,
            start,
        );
        let failed = start + second * 3;
        log.record(
            "PostToolUseFailure",
            (Some("Bash"), None, None),
            None,
            failed,
        );
        log.record(
            "Stop",
            (None, Some("done"), None),
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
            log.record("UserPromptSubmit", (None, None, None), None, start);
        }
        assert_eq!(log.entries.len(), KEPT, "old steps drop");
    }

    #[test]
    fn prompts_start_rounds_and_ends_close_them() {
        let start = Instant::now();
        let at = |n: u64| start + Duration::from_secs(n);
        let mut log = Timeline::default();
        log.record(
            "PreToolUse",
            (Some("Read"), Some("a.rs"), None),
            None,
            at(0),
        );
        log.record("UserPromptSubmit", (None, Some("one"), None), None, at(1));
        log.record(
            "PreToolUse",
            (Some("Edit"), Some("b.rs"), None),
            None,
            at(2),
        );
        log.record("Stop", (None, Some("did one"), None), Some(at(1)), at(4));
        log.record("UserPromptSubmit", (None, None, None), None, at(5));
        log.record("Notification", (None, Some("may I?"), None), None, at(6));
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

    #[test]
    fn results_find_their_own_calls_in_any_order() {
        let start = Instant::now();
        let at = |n: u64| start + Duration::from_secs(n);
        let mut log = Timeline::default();
        log.record(
            "PreToolUse",
            (Some("Read"), Some("a"), Some("1")),
            None,
            at(0),
        );
        log.record(
            "PreToolUse",
            (Some("Read"), Some("b"), Some("2")),
            None,
            at(1),
        );
        assert!(log.record(
            "PostToolUse",
            (Some("Read"), Some("b"), Some("2")),
            None,
            at(3)
        ));
        assert!(log.record(
            "PostToolUse",
            (Some("Read"), Some("a"), Some("1")),
            None,
            at(9)
        ));
        let took: Vec<_> = (log.entries.iter())
            .map(|entry| match &entry.step {
                Step::Tool { took, .. } => took.map(|took| took.as_secs()),
                _ => None,
            })
            .collect();
        assert_eq!(took, [Some(9), Some(2)]);
        assert!(
            !log.record(
                "PostToolUse",
                (Some("Read"), Some("a"), Some("9")),
                None,
                at(9)
            ),
            "no call has that id"
        );
    }

    #[test]
    fn without_ids_a_result_goes_by_its_tool_and_detail() {
        let start = Instant::now();
        let mut log = Timeline::default();
        log.record("PreToolUse", (Some("Read"), Some("a"), None), None, start);
        log.record("PreToolUse", (Some("Grep"), Some("b"), None), None, start);
        log.record("PostToolUse", (Some("Read"), Some("a"), None), None, start);
        let open = |log: &Timeline, at: usize| {
            matches!(log.entries[at].step, Step::Tool { took: None, .. })
        };
        assert!(!open(&log, 0) && open(&log, 1));
    }

    #[test]
    fn a_turn_no_one_typed_is_its_own_round() {
        let start = Instant::now();
        let at = |n: u64| start + Duration::from_secs(n);
        let mut log = Timeline::default();
        log.record("UserPromptSubmit", (None, Some("one"), None), None, at(0));
        log.record("Stop", (None, Some("did"), None), Some(at(0)), at(1));
        log.record(
            "PreToolUse",
            (Some("Read"), Some("woken"), None),
            None,
            at(50),
        );
        log.record("Stop", (None, Some("did more"), None), Some(at(50)), at(51));
        let rounds = log.rounds();
        assert_eq!(rounds.len(), 2);
        assert_eq!(rounds[1].prompt, None);
        assert_eq!(rounds[1].steps.len(), 1);
        assert_eq!(rounds[0].end.map(|end| end.2), Some(Some("did")));
    }

    #[test]
    fn one_question_and_one_end_show_once() {
        let start = Instant::now();
        let mut log = Timeline::default();
        log.record("PermissionRequest", (None, Some("rm"), None), None, start);
        log.record("Notification", (None, Some("needs you"), None), None, start);
        assert_eq!(log.entries.len(), 1);
        assert_eq!(log.entries[0].step, Step::Asked(Some("rm".into())));
        log.record("Stop", (None, None, None), Some(start), start);
        log.record("agent-turn-complete", (None, None, None), None, start);
        assert_eq!(log.entries.len(), 2);
    }
}
