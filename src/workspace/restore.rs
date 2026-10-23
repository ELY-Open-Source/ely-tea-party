//! Sessions outlive the app: each one's folder and its agent's conversation are saved, and come back at the next start.

use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::Context as _;
use gpui::{Context, Window};
use serde::{Deserialize, Serialize};

use super::{Workspace, start};
use crate::agents;

/// A shell this quiet has drawn its prompt.
const SETTLED: Duration = Duration::from_secs(2);

/// What the next start brings back.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Saved {
    pub sessions: Vec<Place>,
    /// The session shown.
    pub active: usize,
}

/// A session as it is saved: its folder, the line that resumes its agent's conversation, and what sets it apart.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Place {
    pub folder: PathBuf,
    pub resume: Option<String>,
    #[serde(default)]
    pub name: String,
    /// None in a file saved before trinkets: it takes a free one.
    #[serde(default)]
    pub trinket: Option<usize>,
}

/// The saved sessions' file, beside the settings; a dev build keeps its own, so it never takes a release's sessions.
fn path() -> anyhow::Result<PathBuf> {
    let folder = dirs::config_dir().context("restore: the platform names no config folder")?;
    let dev = cfg!(debug_assertions) || option_env!("ELY_DEV").is_some();
    let name = if dev {
        "sessions-dev.json"
    } else {
        "sessions.json"
    };
    Ok(folder.join("ely-tea-party").join(name))
}

/// The saved sessions, if any were; a file that does not parse is logged and passed by, so the app still starts.
pub fn load() -> Option<Saved> {
    let path = path().map_err(|error| log::error!("{error:#}")).ok()?;
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            log::error!("restore: cannot read {}: {error}", path.display());
            return None;
        }
    };
    match serde_json::from_str::<Saved>(&text) {
        Ok(saved) => {
            log::info!(
                "restore: {} sessions saved in {}",
                saved.sessions.len(),
                path.display()
            );
            Some(saved)
        }
        Err(error) => {
            log::error!(
                "restore: {} does not parse, so nothing comes back: {error}",
                path.display()
            );
            None
        }
    }
}

fn save(saved: &Saved) -> anyhow::Result<()> {
    let path = path()?;
    let folder = path.parent().context("restore: the file has no folder")?;
    std::fs::create_dir_all(folder)
        .with_context(|| format!("restore: cannot make {}", folder.display()))?;
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, serde_json::to_string_pretty(saved)? + "\n")
        .with_context(|| format!("restore: cannot write {}", temp.display()))?;
    std::fs::rename(&temp, &path)
        .with_context(|| format!("restore: cannot replace {}", path.display()))?;
    Ok(())
}

/// The line that resumes `agent`'s `conversation`, for agents that have one, when the id holds no shell syntax.
pub fn resume(agent: &str, conversation: &str) -> Option<String> {
    let plain = !conversation.is_empty()
        && conversation
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'));
    let flag = match agent {
        "claude" | "gemini" | "kimi" => "--resume",
        "codex" => "resume",
        "grok" => "-r",
        "opencode" | "pi" | "omp" => "--session",
        "agy" | "antigravity" => "--conversation",
        _ => return None,
    };
    plain.then(|| format!("{agent} {flag} {conversation}"))
}

impl Workspace {
    fn snapshot(&self) -> Saved {
        let place = |session: &crate::sessions::Session| Place {
            folder: session.folder.clone(),
            // A resume not yet typed stays saved.
            resume: session.pending.clone().or_else(|| {
                let agent = session.agent.as_ref()?;
                resume(agent.program.as_deref()?, agent.conversation.as_deref()?)
            }),
            name: session.name.clone(),
            trinket: Some(session.trinket),
        };
        Saved {
            sessions: self.sessions.iter().map(place).collect(),
            active: self.active,
        }
    }

    /// Saves the sessions when they changed since the last save.
    pub(super) fn keep(&mut self) {
        let now = self.snapshot();
        if self.saved.as_ref() == Some(&now) {
            return;
        }
        match save(&now) {
            Ok(()) => log::debug!("restore: {} sessions saved", now.sessions.len()),
            Err(error) => log::error!("{error:#}"),
        }
        self.saved = Some(now);
    }

    /// Types each restored resume whose shell has been quiet a while, for shells that report no folder.
    pub(super) fn type_pending(&mut self, now: Instant, cx: &mut Context<Self>) {
        for session in &mut self.sessions {
            let view = session.terminal.read(cx);
            if session.pending.is_none() || now.saturating_duration_since(view.drew()) < SETTLED {
                continue;
            }
            if let Some(line) = session.pending.take() {
                log::info!(
                    "restore: session {} runs {line}, its shell quiet",
                    session.id
                );
                view.type_line(&line);
            }
        }
    }

    /// Brings `saved` back: the first session runs already in its folder; the rest start, and each agent resumes.
    pub fn restore(&mut self, saved: Saved, window: &mut Window, cx: &mut Context<Self>) {
        let mut places = saved.sessions.into_iter();
        if let (Some(first), Some(session)) = (places.next(), self.sessions.first_mut()) {
            session.pending = first.resume.clone();
            dress(session, first);
        }
        let shell = crate::settings::SettingsStore::get(cx).shell.clone();
        for place in places {
            if !place.folder.is_dir() {
                log::info!(
                    "restore: {} is gone; its session stays closed",
                    place.folder.display()
                );
                continue;
            }
            let id = agents::next_session();
            match start(&shell, place.folder.clone(), Some(id), cx) {
                Ok(((pty, events), launch)) => {
                    self.add((id, pty, events), launch, window, cx);
                    if let Some(session) = self.sessions.last_mut() {
                        session.pending = place.resume.clone();
                        dress(session, place);
                        // Out of view until shown, so it posts notifications.
                        session.terminal.update(cx, |view, _| view.set_front(false));
                    }
                }
                Err(error) => log::error!("restore: a session did not start: {error:#}"),
            }
        }
        let active = saved.active.min(self.sessions.len().saturating_sub(1));
        log::info!("restore: {} sessions back", self.sessions.len());
        self.select(active, window, cx);
    }
}

/// Gives a restored session the name and trinket `place` kept.
fn dress(session: &mut crate::sessions::Session, place: Place) {
    session.name = place.name;
    if let Some(trinket) = place.trinket {
        session.trinket = trinket;
    }
}

/// The folder the first session starts in when `saved` comes back: its first session's, while it exists.
pub fn first_folder(saved: &Saved) -> Option<&Path> {
    saved
        .sessions
        .first()
        .map(|place| place.folder.as_path())
        .filter(|folder| folder.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_agent_resumes_its_own_way_and_odd_ids_do_not_reach_the_shell() {
        assert_eq!(
            resume("claude", "a1-b2").as_deref(),
            Some("claude --resume a1-b2")
        );
        assert_eq!(
            resume("codex", "019a").as_deref(),
            Some("codex resume 019a")
        );
        assert_eq!(
            resume("agy", "c.1").as_deref(),
            Some("agy --conversation c.1")
        );
        assert_eq!(resume("dscode", "x"), None, "no resume known");
        assert_eq!(resume("claude", "x; rm -rf ~"), None);
        assert_eq!(resume("claude", ""), None);
    }

    #[test]
    fn a_saved_file_reads_back() {
        let saved = Saved {
            sessions: vec![Place {
                folder: "/tmp".into(),
                resume: Some("claude --resume a".into()),
                name: String::new(),
                trinket: Some(2),
            }],
            active: 0,
        };
        let text = serde_json::to_string(&saved).expect("written");
        assert_eq!(serde_json::from_str::<Saved>(&text).expect("read"), saved);
    }

    #[test]
    fn a_file_saved_before_names_and_trinkets_still_reads() {
        let text = r#"{"sessions":[{"folder":"/tmp","resume":null}],"active":0}"#;
        let saved = serde_json::from_str::<Saved>(text).expect("read");
        assert_eq!(saved.sessions[0].trinket, None);
        assert!(saved.sessions[0].name.is_empty());
    }
}
