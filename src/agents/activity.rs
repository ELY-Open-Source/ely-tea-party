//! What a hooked agent is doing, from its hooks' tools and words, as its character shows it.

use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

use super::{
    Agent, Pose, Status,
    quips::{self, Moment},
    timeline::Timeline,
};

/// A command running this long lets its character slack until it ends.
const SLACK_AFTER: Duration = Duration::from_secs(20);
/// A turn this long ends in its character's finale.
const CHEER_AFTER: Duration = Duration::from_secs(30);
/// How long the finale plays.
const CHEER_FOR: Duration = Duration::from_secs(8);
/// How long a character's word stays up.
const QUIP_FOR: Duration = Duration::from_secs(6);

/// What a tool does, as its character shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Doing {
    Reading,
    Searching,
    Browsing,
    Editing,
    Running,
    Delegating,
    Planning,
    Other,
}

impl Doing {
    /// What tool `name` does, in any agent's spelling; tools it does not know just work.
    pub fn of(name: &str) -> Doing {
        match name.to_ascii_lowercase().as_str() {
            "read" | "read_file" | "readfile" | "read_many_files" | "readmediafile"
            | "notebookread" | "ls" | "list_directory" | "view" | "view_file" => Doing::Reading,
            "grep"
            | "glob"
            | "find"
            | "search_file_content"
            | "grep_search"
            | "find_by_name"
            | "codebase_search"
            | "toolsearch"
            | "lsp" => Doing::Searching,
            "webfetch" | "websearch" | "web_fetch" | "google_web_search" | "searchweb"
            | "fetchurl" | "read_url_content" | "search_web" => Doing::Browsing,
            "edit"
            | "multiedit"
            | "write"
            | "notebookedit"
            | "write_file"
            | "replace"
            | "strreplacefile"
            | "writefile"
            | "apply_patch"
            | "patch"
            | "write_to_file"
            | "replace_file_content"
            | "multi_replace_file_content" => Doing::Editing,
            "bash" | "bashoutput" | "powershell" | "monitor" | "shell" | "run_shell_command"
            | "exec_command" | "local_shell" | "run_command" | "unified_exec" => Doing::Running,
            "task" | "agent" | "subagent" | "invoke_subagent" | "team_spawn" => Doing::Delegating,
            "todowrite" | "todoread" | "taskcreate" | "taskupdate" | "enterplanmode"
            | "exitplanmode" | "write_todos" | "settodolist" | "update_plan" => Doing::Planning,
            _ => Doing::Other,
        }
    }

    /// The verb before its detail in a row.
    pub fn verb(self) -> &'static str {
        match self {
            Doing::Reading => "读",
            Doing::Searching => "搜",
            Doing::Browsing => "查网页",
            Doing::Editing => "改",
            Doing::Running => "跑",
            Doing::Delegating => "派分身",
            Doing::Planning => "列计划",
            Doing::Other => "用",
        }
    }

    fn pose(self) -> Pose {
        match self {
            Doing::Reading | Doing::Searching | Doing::Browsing => Pose::Research,
            Doing::Editing | Doing::Running | Doing::Other => Pose::Working,
            Doing::Delegating => Pose::Special,
            Doing::Planning => Pose::Thinking,
        }
    }
}

/// The tool a hooked agent last reached for.
#[derive(Clone, Debug)]
pub struct Activity {
    pub doing: Doing,
    /// The tool's name, shown when it does nothing the app knows.
    pub tool: String,
    pub detail: Option<String>,
    pub since: Instant,
    /// Its result came back.
    pub done: bool,
}

/// One turn, from the prompt to the agent's stop.
#[derive(Clone, Debug, Default)]
pub struct Turn {
    pub started: Option<Instant>,
    pub tools: u32,
    /// The names of the files it edited.
    pub edited: BTreeSet<String>,
    /// When it ended long enough after it started to cheer.
    pub cheered: Option<Instant>,
    /// What the agent last asked or said.
    pub said: Option<String>,
}

impl Agent {
    /// Takes in hook `event`'s tool and detail, writing it into its session's `log`.
    pub fn note(
        &mut self,
        event: &str,
        (tool, detail): (Option<&str>, Option<&str>),
        log: &mut Timeline,
        now: Instant,
    ) {
        log.record(event, (tool, detail), self.turn.started, now);
        let detail = detail.map(str::to_string);
        match event {
            "UserPromptSubmit" => {
                self.turn = Turn {
                    started: Some(now),
                    ..Turn::default()
                };
                self.activity = None;
                self.say(Moment::Received, now);
            }
            "PreToolUse" => {
                let tool = tool.unwrap_or_default();
                let doing = Doing::of(tool);
                self.turn.tools += 1;
                if let (Doing::Editing, Some(file)) = (doing, &detail) {
                    self.turn.edited.insert(file.clone());
                }
                if doing == Doing::Delegating {
                    self.say(Moment::Delegates, now);
                }
                self.activity = Some(Activity {
                    doing,
                    tool: tool.to_string(),
                    detail,
                    since: now,
                    done: false,
                });
            }
            "PostToolUse" | "PostToolUseFailure" => match self.activity.as_mut() {
                Some(activity) if !activity.done => {
                    activity.done = true;
                    let long = activity.doing == Doing::Running
                        && now.saturating_duration_since(activity.since) >= SLACK_AFTER;
                    if event == "PostToolUseFailure" {
                        self.say(Moment::Failed, now);
                    } else if long {
                        self.say(Moment::LongDone, now);
                    }
                }
                // Agents that hook no PreToolUse, as Antigravity, name the tool afterwards.
                _ => {
                    if let Some(tool) = tool {
                        self.note("PreToolUse", (Some(tool), detail.as_deref()), log, now);
                        self.note(event, (None, None), log, now);
                    }
                }
            },
            "Notification" | "PermissionRequest" => {
                self.turn.said = detail;
                self.say(Moment::Asks, now);
            }
            "Stop" | "StopFailure" | "agent-turn-complete" => {
                let long = self
                    .turn
                    .started
                    .is_some_and(|started| now.saturating_duration_since(started) >= CHEER_AFTER);
                self.turn.cheered = (long && event != "StopFailure").then_some(now);
                self.turn.said = detail;
                self.activity = None;
                if event == "StopFailure" {
                    self.say(Moment::Failed, now);
                } else if long {
                    self.say(Moment::Finale, now);
                }
            }
            _ => {}
        }
    }

    fn say(&mut self, moment: Moment, now: Instant) {
        self.quip = Some((moment, now));
    }

    /// What she says at `now`, if a moment was recent enough.
    pub fn bubble(&self, now: Instant) -> Option<(Moment, &'static str)> {
        let (moment, at) = self.quip?;
        (now.saturating_duration_since(at) < QUIP_FOR)
            .then(|| (moment, quips::line(self.kind, moment)))
    }

    /// A hooked agent's pose at `now`, its last hook `quiet` ago, from what it does.
    pub(super) fn hooked_pose(&self, now: Instant, quiet: Duration, thinking: Duration) -> Pose {
        match (self.status, &self.activity) {
            (Status::Working, Some(activity)) if !activity.done => {
                let running = now.saturating_duration_since(activity.since);
                if activity.doing == Doing::Running && running >= SLACK_AFTER {
                    Pose::Slacking
                } else {
                    activity.doing.pose()
                }
            }
            (Status::Working, Some(activity)) if quiet < thinking => activity.doing.pose(),
            (Status::Working, _) => Pose::Thinking,
            (_, _) if self.cheering(now) => Pose::Special,
            (Status::NeedsInput, _) => Pose::NeedsInput,
            (Status::Idle, _) => Pose::Idle,
        }
    }

    fn cheering(&self, now: Instant) -> bool {
        self.status == Status::Idle
            && self
                .turn
                .cheered
                .is_some_and(|at| now.saturating_duration_since(at) < CHEER_FOR)
    }

    /// What a row says the agent is doing in `pose`, beyond its character's label.
    pub fn news(&self, pose: Pose) -> Option<String> {
        let edited = self.turn.edited.len();
        match (self.status, pose) {
            (Status::NeedsInput, _) => self.turn.said.clone(),
            (Status::Working, Pose::Thinking | Pose::Idle) => None,
            (Status::Working, _) => self.activity.as_ref().map(|activity| {
                let what = match (&activity.detail, activity.doing) {
                    (Some(detail), _) => detail.clone(),
                    (None, Doing::Other) => activity.tool.clone(),
                    (None, _) => String::new(),
                };
                let waits = pose == Pose::Slacking;
                let verb = if waits { "等" } else { activity.doing.verb() };
                let tail = if waits { " 跑完" } else { "" };
                format!("{verb} {what}{tail}").trim_end().to_string()
            }),
            (_, Pose::Sleeping) => None,
            (Status::Idle, _) if edited > 0 => Some(format!("这轮改了 {edited} 个文件")),
            (Status::Idle, _) => self.turn.said.clone(),
        }
    }
}

#[cfg(test)]
mod tests;
