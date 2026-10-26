use gpui::{Context, EntityId, Window};

use super::Workspace;
use crate::{
    agents,
    settings::{self, SettingsStore, StartIn},
    term::shell::Start,
};

impl Workspace {
    /// A session of the set shell, where Start in says.
    pub(super) fn new_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let settings = SettingsStore::get(cx);
        let start = match settings.start_in {
            StartIn::Home => Start::Home,
            StartIn::Session => self
                .sessions
                .get(self.active)
                .map_or(Start::Home, |session| Start::In(session.folder.clone())),
            StartIn::Folder => match settings::start_folder(&settings.start_folder) {
                Ok(folder) => Start::In(folder),
                Err(error) => return log::error!("app: no new session: {error:#}"),
            },
        };
        let shell = settings.shell.clone();
        self.open(&shell, start, None, window, cx);
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
        self.open(&shell, Start::In(folder), Some(index + 1), window, cx);
    }

    /// Shows a `shell` session from `start`, at `at` or last.
    fn open(
        &mut self,
        shell: &str,
        start: Start,
        at: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = agents::next_session();
        match super::start(shell, start, Some(id), cx) {
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
