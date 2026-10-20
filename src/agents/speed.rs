//! Session speed, independent of reasoning effort and activity.

mod startup;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Kind, Status};

impl super::Agent {
    pub fn speed_label(&self) -> Option<&'static str> {
        match (self.speed, self.speed_inferred) {
            (Speed::Fast, true) => Some("Fast (configured)"),
            (Speed::Ultrafast, true) => Some("Ultrafast (configured)"),
            (Speed::Fast, false) => Some("Fast"),
            (Speed::Ultrafast, false) => Some("Ultrafast"),
            _ => None,
        }
    }

    pub fn startup_speed(&mut self, pid: u32, cwd: &std::path::Path) {
        if self.kind != Kind::Gpt || self.speed_pid == Some(pid) {
            return;
        }
        let previous = self.speed_pid.replace(pid);
        if previous.is_some() {
            self.speed = Speed::Unknown;
            self.speed_inferred = false;
        }
        if self.speed != Speed::Unknown {
            return;
        }
        let Some(process) = super::detect::process::read(pid) else {
            log::debug!("agents: cannot read Codex startup process {pid}");
            return;
        };
        match startup::resolve(&process, cwd) {
            Ok(Some(speed)) => {
                self.speed = speed;
                self.speed_inferred = true;
                log::info!("agents: Codex process {pid} configured startup speed is {speed:?}");
            }
            Ok(None) => log::debug!("agents: no unambiguous Codex startup speed for {pid}"),
            Err(error) => log::warn!("agents: Codex startup speed config failed: {error}"),
        }
    }

    pub fn reported_speed(&mut self, speed: Speed) -> bool {
        let changed = self.speed != speed || self.speed_inferred;
        self.speed = speed;
        self.speed_inferred = false;
        changed
    }

    pub fn observe_speed(
        &mut self,
        kind: Option<Kind>,
        conversation: Option<&str>,
        speed: Option<Speed>,
    ) -> bool {
        if kind != Some(self.kind) || conversation != self.conversation.as_deref() {
            return false;
        }
        let Some(speed) = speed else {
            return false;
        };
        self.reported_speed(speed)
    }

    pub fn codex_title(&mut self, title: &str) {
        if !title.starts_with("codex | ") {
            return;
        }
        let before = self.speed;
        match title.rsplit(" | ").next() {
            Some("Fast on") => {
                self.reported_speed(Speed::Fast);
            }
            // Codex 0.163 also says Fast off for Ultrafast.
            Some("Fast off") if self.speed == Speed::Fast => {
                self.reported_speed(Speed::Unknown);
            }
            _ => {}
        }
        if self.speed != before {
            log::info!("agents: Codex title speed is {:?}", self.speed);
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Speed {
    #[default]
    Unknown,
    Standard,
    Fast,
    Ultrafast,
}

impl Speed {
    fn tier(tier: &str) -> Self {
        match tier {
            "fast" | "priority" => Self::Fast,
            "ultrafast" => Self::Ultrafast,
            "default" | "standard" => Self::Standard,
            _ => Self::Unknown,
        }
    }

    pub fn read(agent: &str, payload: &Value) -> Option<Self> {
        match Kind::named(agent)? {
            Kind::Claude => payload["fast_mode"]
                .as_bool()
                .map(|fast| if fast { Self::Fast } else { Self::Standard }),
            Kind::Gpt => payload["service_tier"].as_str().map(Self::tier),
            _ => None,
        }
    }

    pub fn running(self, status: Status) -> bool {
        status == Status::Working && matches!(self, Self::Fast | Self::Ultrafast)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn live_reports_replace_the_startup_estimate_without_reloading_it() {
        let mut agent = super::super::Agent::new(Kind::Gpt, std::time::Instant::now());
        agent.speed_pid = Some(42);
        agent.speed = Speed::Ultrafast;
        agent.speed_inferred = true;
        assert!(agent.reported_speed(Speed::Standard));
        assert!(!agent.speed_inferred);
        agent.startup_speed(42, std::path::Path::new("/"));
        assert_eq!(agent.speed, Speed::Standard);
        agent.codex_title("codex | Fast on");
        assert_eq!(agent.speed, Speed::Fast);
        agent.startup_speed(42, std::path::Path::new("/"));
        assert_eq!(agent.speed, Speed::Fast);
    }

    #[test]
    fn metadata_neither_wakes_an_agent_nor_crosses_sessions() {
        let now = std::time::Instant::now();
        let mut agent = super::super::Agent::new(Kind::Claude, now);
        agent.conversation = Some("current".into());
        assert!(!agent.observe_speed(Some(Kind::Claude), Some("old"), Some(Speed::Fast)));
        assert!(!agent.observe_speed(Some(Kind::Gpt), Some("current"), Some(Speed::Ultrafast)));
        assert!(agent.observe_speed(Some(Kind::Claude), Some("current"), Some(Speed::Fast)));
        assert_eq!(agent.status, Status::Idle);
        assert_eq!(agent.active, now);
        assert!(!agent.hooked);
        assert!(!agent.observe_speed(Some(Kind::Claude), Some("current"), None));
        assert_eq!(agent.speed, Speed::Fast);
    }

    #[test]
    fn codex_fast_off_does_not_mean_ultrafast_is_off() {
        let mut agent = super::super::Agent::new(Kind::Gpt, std::time::Instant::now());
        agent.codex_title("codex | project | Fast on");
        assert_eq!(agent.speed, Speed::Fast);
        agent.codex_title("codex | project | Fast off");
        assert_eq!(agent.speed, Speed::Unknown);
        agent.speed = Speed::Ultrafast;
        agent.codex_title("codex | project | Fast off");
        assert_eq!(agent.speed, Speed::Ultrafast);
        agent.codex_title("a document about Fast on");
        assert_eq!(agent.speed, Speed::Ultrafast);
    }

    #[test]
    fn only_explicit_session_speed_counts() {
        assert_eq!(
            Speed::read("claude", &json!({"fast_mode": true})),
            Some(Speed::Fast)
        );
        assert_eq!(
            Speed::read("claude", &json!({"fast_mode": false})),
            Some(Speed::Standard)
        );
        assert_eq!(
            Speed::read("claude", &json!({"service_tier": "ultrafast"})),
            None
        );
        assert_eq!(
            Speed::read("codex", &json!({"service_tier": "priority"})),
            Some(Speed::Fast)
        );
        assert_eq!(
            Speed::read("codex", &json!({"service_tier": "ultrafast"})),
            Some(Speed::Ultrafast)
        );
        assert_eq!(
            Speed::read("codex", &json!({"service_tier": "auto"})),
            Some(Speed::Unknown)
        );
        assert_eq!(Speed::read("codex", &json!({"effort": "ultra"})), None);
        assert!(!Speed::Fast.running(Status::Idle));
        assert!(!Speed::Ultrafast.running(Status::NeedsInput));
        assert!(!Speed::Unknown.running(Status::Working));
        assert!(Speed::Fast.running(Status::Working));
    }
}
