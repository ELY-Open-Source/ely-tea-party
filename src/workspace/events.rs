//! What the sessions' terminals report, and presses on the notifications they posted.

use gpui::{Context, EntityId, Window};

use super::Workspace;
use crate::{
    agents::Status,
    term::{State, TerminalEvent},
};

impl Workspace {
    /// What session `id`'s terminal reported.
    pub(super) fn session_event(
        &mut self,
        id: EntityId,
        event: &TerminalEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self
            .sessions
            .iter()
            .position(|session| session.terminal.entity_id() == id)
        else {
            return;
        };
        let session = &mut self.sessions[index];
        match event {
            TerminalEvent::Title(title) => {
                log::debug!("app: session {}'s program names it {title:?}", index + 1);
                session.title.clone_from(title);
                if let Some(agent) = session.agent.as_mut()
                    && agent.kind == crate::agents::Kind::Gpt
                {
                    agent.codex_title(title);
                }
            }
            TerminalEvent::Speed(speed) => {
                if let Some(agent) = session.agent.as_mut()
                    && agent.kind == crate::agents::Kind::Gpt
                    && agent.reported_speed(*speed)
                {
                    log::info!("agents: session {} speed is {speed:?}", session.id);
                }
            }
            TerminalEvent::Folder(folder) => {
                session.folder.clone_from(folder);
                session.reports = true;
                // Its first prompt: the shell takes the restored agent's resume.
                if let Some(line) = session.pending.take() {
                    log::info!("restore: session {} runs {line}", session.id);
                    session.terminal.read(cx).type_line(&line);
                }
                if index == self.active {
                    self.files.update(cx, |files, cx| files.show(folder, cx));
                    self.git.update(cx, |git, cx| git.show(folder, cx));
                }
            }
            TerminalEvent::Bell => {
                if let Some(agent) = session.agent.as_mut().filter(|agent| !agent.hooked) {
                    agent.set(Status::NeedsInput, std::time::Instant::now());
                }
                if index != self.active && !session.bell {
                    log::debug!("app: a bell in hidden session {}", index + 1);
                    session.bell = true;
                }
            }
            // A hidden session that starts waiting marks its row until it is shown.
            TerminalEvent::Shows(Some(State::Blocked)) if index != self.active => {
                log::debug!("app: hidden session {} waits", index + 1);
                session.waiting = true;
            }
            TerminalEvent::Shows(_) => {}
            TerminalEvent::Exited(status) => {
                let pid = session.terminal.read(cx).pid();
                log::info!("app: session {pid}'s program ended, {status}");
                self.close(index, window, cx);
                return;
            }
        }
        if index == self.active {
            self.retitle(window);
        }
        cx.notify();
    }

    /// A press on notification `tag`: its session hears of it and shows, unless it asked not to; false when no session here posted it.
    pub fn pressed(&mut self, tag: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let found = self
            .sessions
            .iter()
            .enumerate()
            .find_map(|(index, session)| {
                let focus = session.terminal.update(cx, |view, _| view.pressed(tag))?;
                Some((index, focus))
            });
        let Some((index, focus)) = found else {
            return false;
        };
        if focus {
            cx.activate(true);
            window.activate_window();
            self.select(index, window, cx);
        }
        true
    }
}
