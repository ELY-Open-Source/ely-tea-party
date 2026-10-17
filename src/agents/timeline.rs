//! What a hooked agent did, in order: each prompt, tool, question and end, as the Activity tab lists them.

use std::time::{Duration, Instant};

use super::{Agent, activity::Doing};

/// The most steps an agent keeps; older ones drop.
pub const KEPT: usize = 200;

/// One step of an agent's work.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Prompt,
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
    },
}

/// A step and when it began.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub at: Instant,
    pub step: Step,
}

impl Agent {
    /// Writes hook `event` into the agent's log.
    pub(super) fn record(
        &mut self,
        event: &str,
        tool: Option<&str>,
        detail: Option<&str>,
        now: Instant,
    ) {
        let step = match event {
            "UserPromptSubmit" => Step::Prompt,
            "PreToolUse" => {
                let name = tool.unwrap_or_default().to_string();
                Step::Tool {
                    doing: Doing::of(&name),
                    name,
                    detail: detail.map(str::to_string),
                    took: None,
                    failed: false,
                }
            }
            "PostToolUse" | "PostToolUseFailure" => {
                let open = self
                    .log
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
            "Notification" | "PermissionRequest" => Step::Asked(detail.map(str::to_string)),
            "Stop" | "StopFailure" | "agent-turn-complete" => Step::Ended {
                failed: event == "StopFailure",
                took: self
                    .turn
                    .started
                    .map(|started| now.saturating_duration_since(started)),
            },
            _ => return,
        };
        if self.log.len() == KEPT {
            self.log.pop_front();
        }
        self.log.push_back(Entry { at: now, step });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::Kind;

    #[test]
    fn steps_keep_their_order_and_tools_their_time() {
        let start = Instant::now();
        let mut agent = Agent::new(Kind::Claude, start);
        let second = Duration::from_secs(1);
        agent.note("UserPromptSubmit", None, None, start);
        agent.note("PreToolUse", Some("Bash"), Some("cargo test"), start);
        agent.note("PostToolUseFailure", Some("Bash"), None, start + second * 3);
        agent.note("Stop", None, None, start + second * 5);
        let steps: Vec<&Step> = agent.log.iter().map(|entry| &entry.step).collect();
        assert_eq!(steps[0], &Step::Prompt);
        assert!(matches!(
            steps[1],
            Step::Tool { took: Some(took), failed: true, .. } if *took == second * 3
        ));
        assert_eq!(
            steps[2],
            &Step::Ended {
                failed: false,
                took: Some(second * 5)
            }
        );
        for _ in 0..KEPT {
            agent.note("UserPromptSubmit", None, None, start);
        }
        assert_eq!(agent.log.len(), KEPT, "old steps drop");
    }
}
