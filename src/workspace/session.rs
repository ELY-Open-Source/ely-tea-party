use std::{path::PathBuf, time::Instant};

use anyhow::Context as _;
use gpui::{Context, Window};

use super::Workspace;
use crate::{
    agents,
    sessions::{self, Layout, Program, Session, orb},
    settings::{self, SettingsStore, StartIn},
    term::{
        Events, Launch, Pty, config,
        shell::{ShellIntegration, Start, command, launch},
        spawn,
    },
};

impl Workspace {
    /// Notes each session's mark `now`, and draws the window at each step while one shown moves.
    pub(super) fn tick(&mut self, now: Instant, cx: &mut Context<Self>) {
        let mut moving = false;
        for session in &mut self.sessions {
            moving |= session.note(now, cx);
        }
        let moving = moving && SettingsStore::get(cx).show_sessions;
        if moving == self.ticking.is_some() {
            return;
        }
        log::debug!("app: status marks {}", if moving { "move" } else { "rest" });
        self.ticking = moving.then(|| {
            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(orb::wait(Instant::now()))
                        .await;
                    if this.update(cx, |_, cx| cx.notify()).is_err() {
                        break;
                    }
                }
            })
        });
    }

    /// Keeps session `id`, its first pane started to run `program`, as the last row.
    pub(super) fn add(
        &mut self,
        id: u64,
        started: (Pty, Events, Launch),
        program: Program,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let pane = self.pane(started, program, window, cx);
        let layout = Layout::Pane(pane.id());
        log::info!("app: session {} opens", self.sessions.len() + 1);
        let trinket = sessions::free_trinket(self.sessions.iter().map(|session| session.trinket));
        // A new session joins the shown one's group.
        let group = self.sessions.get(self.active).and_then(|shown| shown.group);
        self.sessions.push(Session {
            id,
            link_id: Default::default(),
            agent: None,
            panes: vec![pane],
            layout,
            focused: 0,
            name: String::new(),
            editing: None,
            bell: false,
            waiting: false,
            look: None,
            log: Default::default(),
            trinket,
            drawer: None,
            pending: None,
            group,
        });
    }

    /// A session of the set shell, where Start in says.
    pub(super) fn new_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(start) = self.start_in(cx) else {
            return;
        };
        let program = Program {
            shell: SettingsStore::get(cx).shell.clone(),
            ..Program::default()
        };
        if let Err(error) = self.open(program, start, None, window, cx) {
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
        let program = Program {
            shell,
            args,
            written: false,
        };
        if let Err(error) = self.open(program, start, None, window, cx) {
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
                    .map_or(Start::Home, |session| {
                        Start::In(session.pane().folder.clone())
                    }),
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
    pub fn duplicate(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some((program, folder, index)) = self
            .sessions
            .iter()
            .enumerate()
            .find(|(_, session)| session.id == id)
            .map(|(index, session)| {
                let pane = session.pane();
                (pane.program.clone(), pane.folder.clone(), index)
            })
        else {
            log::error!("app: there is no session {id:?} to duplicate");
            return;
        };
        log::info!("app: session {id:?} duplicates into {}", folder.display());
        if let Err(error) = self.open(program, Start::In(folder), Some(index + 1), window, cx) {
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
        let program = self
            .sessions
            .get(self.active)
            .map(|session| session.pane().program.clone())
            .context("app: no active session for the chosen folder")?;
        log::info!("app: session here {}", folder.display());
        let at = Some(self.active + 1);
        self.open(program, Start::In(folder), at, window, cx)
    }

    /// Shows a session Windows handed over, last.
    #[cfg(windows)]
    pub fn adopt(
        &mut self,
        started: (Pty, Events, Launch),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Its program runs already, so no hook of it names this session.
        self.add(
            agents::next_session(),
            started,
            Program::default(),
            window,
            cx,
        );
        let id = self.sessions[self.sessions.len() - 1].id;
        self.arrange();
        let index = (self.sessions.iter())
            .position(|session| session.id == id)
            .expect("the session handed over");
        self.select(index, window, cx);
    }

    /// Starts `program` from `start`, its pty sized as the first layout will fit it. With `hooked`, its agents' hooks and messages reach the app as that session's own.
    pub(super) fn started(
        &self,
        program: &Program,
        start: Start,
        hooked: Option<u64>,
        cx: &Context<Self>,
    ) -> anyhow::Result<(Pty, Events, Launch)> {
        let Program {
            shell,
            args,
            written,
        } = program;
        let mut launch = match (written, start) {
            (true, Start::In(folder)) => command(shell, args, folder)?,
            (true, Start::Home) => anyhow::bail!("app: a written program starts in a folder"),
            (false, start) => launch(shell, args, start, &cx.global::<ShellIntegration>().0)?,
        };
        if let Some(id) = hooked {
            launch.env.extend(agents::env(id, false)?);
        }
        let (pty, events) = spawn(&launch, (80, 24), (8, 16), config(SettingsStore::get(cx)))?;
        Ok((pty, events, launch))
    }

    /// Shows a session of `program` from `start`, at `at` or last.
    fn open(
        &mut self,
        program: Program,
        start: Start,
        at: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let id = agents::next_session();
        let started = self.started(&program, start, Some(id), cx)?;
        self.add(id, started, program, window, cx);
        let last = self.sessions.len() - 1;
        if let Some(at) = at.map(|at| at.min(last)) {
            // It joins the group of the row it lands after.
            let before = at
                .checked_sub(1)
                .and_then(|before| self.sessions[before].group);
            self.sessions[last].group = before;
            self.move_session(last, at, cx);
        }
        self.arrange();
        let index = (self.sessions.iter())
            .position(|session| session.id == id)
            .expect("the session opened");
        self.select(index, window, cx);
        Ok(())
    }
}
