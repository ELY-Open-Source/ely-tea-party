use std::path::PathBuf;

use anyhow::Context as _;
use gpui::{AppContext, Context, EntityId, Window};

use super::Workspace;
use crate::{
    agents,
    sessions::{self, Session},
    settings::{self, SettingsStore, StartIn},
    term::{Events, Launch, Pty, TerminalView, shell::Start},
};

impl Workspace {
    /// Keeps session `id`, `pty` started from `launch`, as the last row.
    pub(super) fn add(
        &mut self,
        (id, pty, events): (u64, Pty, Events),
        launch: Launch,
        args: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let terminal = cx.new(|cx| TerminalView::new(pty, events, window, cx));
        let entity = terminal.entity_id();
        let events = cx.subscribe_in(&terminal, window, move |this, _, event, window, cx| {
            this.session_event(entity, event, window, cx)
        });
        let shell =
            (launch.program.as_ref()).map_or_else(String::new, |(program, _)| program.clone());
        let pid = terminal.read(cx).pid();
        log::info!("app: session {pid} opens in {}", launch.cwd.display());
        let trinket = sessions::free_trinket(self.sessions.iter().map(|session| session.trinket));
        self.sessions.push(Session {
            id,
            link_id: Default::default(),
            agent: None,
            terminal,
            title: String::new(),
            name: String::new(),
            editing: None,
            folder: launch.cwd,
            shell,
            args,
            reports: launch.reports,
            bell: false,
            waiting: false,
            look: None,
            log: Default::default(),
            trinket,
            drawer: None,
            pending: None,
            _events: events,
        });
    }

    /// A session of the set shell, where Start in says.
    pub(super) fn new_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(start) = self.start_in(cx) else {
            return;
        };
        let shell = SettingsStore::get(cx).shell.clone();
        if let Err(error) = self.open(&shell, Vec::new(), start, None, window, cx) {
            log::error!("app: no new session: {error:#}");
        }
    }

    /// A profile's session: `shell` with `args`, in `folder` or where Start in says.
    pub fn profile(
        &mut self,
        (shell, args): (String, Vec<String>),
        folder: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let start = match folder {
            Some(folder) => Start::In(folder),
            None => match self.start_in(cx) {
                Some(start) => start,
                None => return,
            },
        };
        log::info!("app: a profile opens {shell} with {} arguments", args.len());
        if let Err(error) = self.open(&shell, args, start, None, window, cx) {
            log::error!("app: no profile session: {error:#}");
        }
    }

    /// Where Start in puts a new session; none when its folder is gone.
    fn start_in(&self, cx: &Context<Self>) -> Option<Start> {
        let settings = SettingsStore::get(cx);
        match settings.start_in {
            StartIn::Home => Some(Start::Home),
            StartIn::Session => Some(
                self.sessions
                    .get(self.active)
                    .map_or(Start::Home, |session| Start::In(session.folder.clone())),
            ),
            StartIn::Folder => match settings::start_folder(&settings.start_folder) {
                Ok(folder) => Some(Start::In(folder)),
                Err(error) => {
                    log::error!("app: no new session: {error:#}");
                    None
                }
            },
        }
    }

    /// Session `id` again: the shell it runs, in the folder it last reported.
    pub fn duplicate(&mut self, id: EntityId, window: &mut Window, cx: &mut Context<Self>) {
        let Some((shell, args, folder, index)) = self
            .sessions
            .iter()
            .enumerate()
            .find(|(_, session)| session.terminal.entity_id() == id)
            .map(|(index, session)| {
                let shell = session.shell.clone();
                (shell, session.args.clone(), session.folder.clone(), index)
            })
        else {
            log::error!("app: there is no session {id:?} to duplicate");
            return;
        };
        log::info!("app: session {id:?} duplicates into {}", folder.display());
        if let Err(error) = self.open(&shell, args, Start::In(folder), Some(index + 1), window, cx)
        {
            log::error!("app: no duplicate session: {error:#}");
        }
    }

    /// The shown shell in `folder`, right after the shown row.
    pub fn here(
        &mut self,
        folder: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let (shell, args) = self
            .sessions
            .get(self.active)
            .map(|session| (session.shell.clone(), session.args.clone()))
            .context("app: no active session for the chosen folder")?;
        log::info!("app: session here {}", folder.display());
        let at = Some(self.active + 1);
        self.open(&shell, args, Start::In(folder), at, window, cx)
    }

    /// Shows a `shell` session with `args` from `start`, at `at` or last.
    fn open(
        &mut self,
        shell: &str,
        args: Vec<String>,
        start: Start,
        at: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let id = agents::next_session();
        let ((pty, events), launch) = super::start(shell, &args, start, Some(id), cx)?;
        self.add((id, pty, events), launch, args, window, cx);
        let last = self.sessions.len() - 1;
        if let Some(at) = at {
            self.move_session(last, at, cx);
        }
        self.select(at.map_or(last, |at| at.min(last)), window, cx);
        Ok(())
    }
}
