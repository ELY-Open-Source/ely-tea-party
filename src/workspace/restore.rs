//! Sessions outlive the app: each one's folder, its agent's conversation, its panes and its group are saved, and come back at the next start.

use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::Context as _;
use gpui::{Context, Window};
use serde::{Deserialize, Serialize};

use super::{Workspace, groups::Group};
use crate::{
    agents,
    sessions::{Program, Session},
    term::shell::Start,
};

mod layout;
pub mod steps;

pub use layout::{Panes, SavedLayout};
use layout::{layout_of, panes_saved};

/// A shell this quiet has drawn its prompt.
const SETTLED: Duration = Duration::from_secs(2);

/// What the next start brings back.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Saved {
    pub sessions: Vec<Place>,
    /// The session shown.
    pub active: usize,
    /// The sessions' groups, in the order their rows show; none in a file saved before groups.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<SavedGroup>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedGroup {
    pub id: u64,
    pub name: String,
}

/// A session as it is saved: its folder, the line that resumes its agent's conversation, and what sets it apart.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Place {
    #[serde(default)]
    pub link_id: crate::sessions::LinkId,
    pub folder: PathBuf,
    pub resume: Option<String>,
    #[serde(default)]
    pub name: String,
    /// None in a file saved before trinkets: it takes a free one.
    #[serde(default)]
    pub trinket: Option<usize>,
    /// The group its row sits in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<u64>,
    /// Its panes, while it holds more than its first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panes: Option<Panes>,
}

/// The saved sessions' file.
fn path() -> anyhow::Result<PathBuf> {
    file("sessions")
}

/// File `stem`, beside the settings; a dev build keeps its own, so it never takes a release's.
fn file(stem: &str) -> anyhow::Result<PathBuf> {
    let folder = crate::settings::folder()?;
    let dev = cfg!(debug_assertions) || option_env!("ELY_DEV").is_some();
    let name = if dev {
        format!("{stem}-dev.json")
    } else {
        format!("{stem}.json")
    };
    Ok(folder.join(name))
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
    replace(&path()?, serde_json::to_string_pretty(saved)? + "\n")
}

/// Writes `text` over `path` whole, through a file beside it.
fn replace(path: &Path, text: String) -> anyhow::Result<()> {
    let folder = path.parent().context("restore: the file has no folder")?;
    std::fs::create_dir_all(folder)
        .with_context(|| format!("restore: cannot make {}", folder.display()))?;
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, text)
        .with_context(|| format!("restore: cannot write {}", temp.display()))?;
    std::fs::rename(&temp, path)
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
        let place = |session: &Session| Place {
            link_id: session.link_id,
            folder: session.main().folder.clone(),
            // A resume not yet typed stays saved.
            resume: session.pending.clone().or_else(|| {
                let agent = session.agent.as_ref()?;
                resume(agent.program.as_deref()?, agent.conversation.as_deref()?)
            }),
            name: session.name.clone(),
            trinket: Some(session.trinket),
            group: session.group,
            panes: panes_saved(session),
        };
        let groups = (self.groups.iter())
            .filter(|group| (self.sessions.iter()).any(|session| session.group == Some(group.id)))
            .map(|group| SavedGroup {
                id: group.id,
                name: group.name.clone(),
            })
            .collect();
        Saved {
            sessions: self.sessions.iter().map(place).collect(),
            active: self.active,
            groups,
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

    /// Saves what the sessions' agents did, when the settings bring sessions back.
    pub(super) fn keep_steps(&mut self, now: Instant, cx: &Context<Self>) {
        if crate::settings::SettingsStore::get(cx).restore_sessions {
            self.steps.keep(&self.sessions, now);
        }
    }

    /// Types each restored resume whose shell has been quiet a while, for shells that report no folder.
    pub(super) fn type_pending(&mut self, now: Instant, cx: &mut Context<Self>) {
        for session in &mut self.sessions {
            let view = session.main().terminal.read(cx);
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

    /// Brings `saved` back: the first session runs already in its folder; the rest start, and each agent resumes. `behind` starts them all after the session shown, which stays shown.
    pub fn restore(
        &mut self,
        mut saved: Saved,
        behind: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut links = std::collections::HashSet::new();
        for place in &mut saved.sessions {
            place.link_id.ensure_unique(&mut links);
        }
        // A group no session names is left out, and a session naming no group listed sits in none.
        let groups: Vec<Group> = (saved.groups.iter())
            .filter(|group| (saved.sessions.iter()).any(|place| place.group == Some(group.id)))
            .map(|group| Group {
                id: group.id,
                name: group.name.clone(),
                editing: None,
            })
            .collect();
        for place in &mut saved.sessions {
            if place
                .group
                .is_some_and(|id| groups.iter().all(|group| group.id != id))
            {
                log::warn!(
                    "restore: a session's group {:?} is listed nowhere",
                    place.group
                );
                place.group = None;
            }
        }
        self.groups = groups;
        let mut kept = steps::load();
        // The session shown, by its id once open.
        let mut shown = None;
        let mut places = saved.sessions.into_iter().enumerate();
        if !behind
            && let (Some((_, first)), Some(session)) = (places.next(), self.sessions.first_mut())
        {
            session.pending = first.resume.clone();
            let panes = first.panes.clone();
            dress(session, first, &mut kept);
            recall(session, cx);
            shown = Some(session.id);
            if let Some(panes) = panes {
                self.reopen(0, &panes, window, cx);
            }
        }
        let shell = crate::settings::SettingsStore::get(cx).shell.clone();
        for (index, place) in places {
            if !place.folder.is_dir() {
                log::info!(
                    "restore: {} is gone; its session stays closed",
                    place.folder.display()
                );
                continue;
            }
            let id = agents::next_session();
            let program = Program {
                shell: shell.clone(),
                ..Program::default()
            };
            match self.started(&program, Start::In(place.folder.clone()), Some(id), cx) {
                Ok(started) => {
                    self.add(id, started, program, window, cx);
                    let last = self.sessions.len() - 1;
                    let session = &mut self.sessions[last];
                    session.pending = place.resume.clone();
                    let panes = place.panes.clone();
                    dress(session, place, &mut kept);
                    recall(session, cx);
                    if index == saved.active {
                        shown = Some(session.id);
                    }
                    if let Some(panes) = panes {
                        self.reopen(last, &panes, window, cx);
                    }
                    // Out of view until shown, so it posts notifications.
                    for pane in &self.sessions[last].panes {
                        pane.terminal
                            .update(cx, |view, _| view.set_front(false, window));
                    }
                }
                Err(error) => log::error!("restore: a session did not start: {error:#}"),
            }
        }
        // A run given a folder keeps it shown.
        let shown = match behind {
            true => self.sessions.first().map(|session| session.id),
            false => shown,
        };
        self.arrange();
        let active = (self.sessions.iter())
            .position(|session| Some(session.id) == shown)
            .unwrap_or(0);
        log::info!("restore: {} sessions back", self.sessions.len());
        self.select(active, window, cx);
    }

    /// Opens session `index`'s other panes as `panes` lays them out, its first pane running already; a pane whose folder went or that does not start is left out.
    fn reopen(&mut self, index: usize, panes: &Panes, window: &mut Window, cx: &mut Context<Self>) {
        let mut ids = Vec::new();
        for (place, (shell, args, folder)) in panes.layout.panes().into_iter().enumerate() {
            if place == panes.main {
                ids.push(Some(self.sessions[index].main().id()));
                continue;
            }
            if !folder.is_dir() {
                log::info!(
                    "restore: {} is gone; its pane stays closed",
                    folder.display()
                );
                ids.push(None);
                continue;
            }
            let program = Program {
                shell: shell.to_string(),
                args: args.to_vec(),
                written: false,
            };
            // A split gets no hook variables, as one opened by hand.
            match self.started(&program, Start::In(folder.to_path_buf()), None, cx) {
                Ok(started) => {
                    let pane = self.pane(started, program, window, cx);
                    ids.push(Some(pane.id()));
                    self.sessions[index].panes.push(pane);
                }
                Err(error) => {
                    log::error!("restore: a pane did not start: {error:#}");
                    ids.push(None);
                }
            }
        }
        let session = &mut self.sessions[index];
        let Some(layout) = layout_of(&panes.layout, &mut ids.iter().copied()) else {
            return;
        };
        let focused = ids.get(panes.focused).copied().flatten();
        session.focused = (session.panes.iter())
            .position(|pane| Some(pane.id()) == focused)
            .unwrap_or(0);
        session.layout = layout;
        log::info!(
            "restore: session {} opens {} panes",
            session.id,
            session.panes.len()
        );
    }
}

/// Gives a restored session the name and trinket `place` kept, and the steps `kept` for it.
fn dress(session: &mut Session, place: Place, kept: &mut steps::Kept) {
    if let Some(log) = steps::take(kept, place.link_id) {
        session.log = log;
    }
    session.link_id = place.link_id;
    session.name = place.name;
    session.group = place.group;
    if let Some(trinket) = place.trinket {
        session.trinket = trinket;
    }
}

/// Gives a restored session's terminal the prompts of the conversation its agent resumes, from the agent's own record, else from the steps kept, so its rail comes back with it.
fn recall(session: &Session, cx: &mut Context<Workspace>) {
    let Some((agent, conversation)) = session
        .pending
        .as_deref()
        .and_then(agents::transcripts::named)
    else {
        return;
    };
    let (agent, conversation) = (agent.to_owned(), conversation.to_owned());
    let kept = session.log.turns();
    let terminal = session.main().terminal.clone();
    cx.spawn(async move |_, cx| {
        let read = cx
            .background_executor()
            .spawn(async move { agents::transcripts::turns(&agent, &conversation) })
            .await;
        let turns = read.filter(|turns| !turns.is_empty()).unwrap_or(kept);
        if !turns.is_empty() {
            terminal.update(cx, |view, cx| view.restore_turns(turns, cx));
        }
    })
    .detach();
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
                link_id: Default::default(),
                folder: "/tmp".into(),
                resume: Some("claude --resume a".into()),
                name: String::new(),
                trinket: Some(2),
                group: Some(1),
                panes: Some(Panes {
                    layout: SavedLayout::Split {
                        down: true,
                        share: 0.3,
                        first: Box::new(pane("/tmp")),
                        second: Box::new(pane("/")),
                    },
                    main: 0,
                    focused: 1,
                }),
            }],
            active: 0,
            groups: vec![SavedGroup {
                id: 1,
                name: "one".into(),
            }],
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
        assert_eq!(saved.sessions[0].group, None);
        assert_eq!(saved.sessions[0].panes, None);
        assert!(saved.groups.is_empty());
    }

    fn pane(folder: &str) -> SavedLayout {
        SavedLayout::Pane {
            shell: "zsh".into(),
            args: Vec::new(),
            folder: folder.into(),
        }
    }
}
