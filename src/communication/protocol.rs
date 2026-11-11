use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const VERSION: u32 = 1;
pub const GUIDE_VERSION: u32 = 1;
pub const SKILL_VERSION: u32 = 1;
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const SKILL: &str = include_str!("../../assets/skills/ely-session-messaging/SKILL.md");
pub const GUIDE: &str = include_str!("../../assets/skills/ely-session-messaging/guide.md");
pub const MAX_FRAME: u64 = 128 * 1024;

pub fn guide() -> Value {
    json!({"app_version": APP_VERSION, "protocol_version": VERSION,
        "guide_version": GUIDE_VERSION, "skill_version": SKILL_VERSION,
        "guide_sha256": super::skills::hash(GUIDE.as_bytes()), "instructions": GUIDE})
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Guide,
    Whoami,
    List {
        agent: Option<String>,
        folder: Option<String>,
        query: Option<String>,
    },
    Send {
        to: String,
        body: String,
        key: String,
    },
    Inbox {
        all: bool,
    },
    Reply {
        message: String,
        body: String,
        key: String,
    },
    Ack {
        message: String,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub protocol: u32,
    pub session: u64,
    pub token: String,
    pub command: Command,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub protocol: u32,
    pub result: Option<Value>,
    pub error: Option<String>,
}

impl Response {
    pub fn from_result(result: anyhow::Result<Value>) -> Self {
        match result {
            Ok(result) => Self {
                protocol: VERSION,
                result: Some(result),
                error: None,
            },
            Err(error) => Self {
                protocol: VERSION,
                result: None,
                error: Some(format!("{error:#}")),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_manifest_matches_embedded_instructions_and_versions() {
        let manifest: Value =
            serde_json::from_str(include_str!("../../assets/skills/bundle.json")).unwrap();
        assert_eq!(manifest["protocol_version"], VERSION);
        assert_eq!(manifest["guide_version"], GUIDE_VERSION);
        assert_eq!(manifest["skill_version"], SKILL_VERSION);
        assert_eq!(
            manifest["skill_sha256"],
            super::super::skills::hash(SKILL.as_bytes())
        );
        assert_eq!(
            manifest["guide_sha256"],
            super::super::skills::hash(GUIDE.as_bytes())
        );
        assert_eq!(guide()["app_version"], env!("CARGO_PKG_VERSION"));
    }
}
