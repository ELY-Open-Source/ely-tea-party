use std::path::PathBuf;

use gpui::{Context, EntityId, Window};

use super::{Workspace, start};
use crate::{
    agents,
    settings::{SettingsStore, StartIn},
};

impl Workspace {
    /// A session of the shell the settings name, in home or the shown session's folder.
    pub(super) fn new_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(home) = dirs::home_dir() else {
            log::error!("app: no home folder, so no new session");
            return;
        };
        let settings = SettingsStore::get(cx);
        let folder = match settings.start_in {
            StartIn::Home => home,
            StartIn::Session => self
                .sessions
                .get(self.active)
                .map_or(home, |session| session.folder.clone()),
        };
        let shell = settings.shell.clone();
        self.open(&shell, folder, None, window, cx);
    }

    /// Session `id` again: the shell it runs, in the folder it last reported.
    pub fn duplicate(&mut self, id: EntityId, window: &mut Window, cx: &mut Context<Self>) {
        let Some((shell, folder, index)) = self
            .sessions
            .iter()
            .enumerate()
            .find(|(_, session)| session.terminal.entity_id() == id)
            .map(|(index, session)| (session.shell.clone(), session.folder.clone(), index))
        else {
            log::error!("app: there is no session {id:?} to duplicate");
            return;
        };
        log::info!("app: session {id:?} duplicates into {}", folder.display());
        self.open(&shell, folder, Some(index + 1), window, cx);
    }

    /// A session of `shell` in `folder`, at `at` or the list's end, and shown.
    fn open(
        &mut self,
        shell: &str,
        folder: PathBuf,
        at: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = agents::next_session();
        match start(shell, folder, Some(id), cx) {
            Ok(((pty, events), launch)) => {
                self.add((id, pty, events), launch, window, cx);
                let last = self.sessions.len() - 1;
                if let Some(at) = at {
                    self.move_session(last, at, cx);
                }
                self.select(at.map_or(last, |at| at.min(last)), window, cx);
            }
            Err(error) => log::error!("app: no new session: {error:#}"),
        }
    }
}
