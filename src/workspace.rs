use std::time::Instant;

use ely_gpui_component::{
    buttons::{ButtonVariant, IconButton},
    lists::List,
    primitives::{FocusScope, IconName},
    shell::{ColumnShell, ResizeBorder, ShellColumn},
    theme::ControlSize,
};
use gpui::{
    App, AppContext, ClickEvent, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Pixels, Render, StatefulInteractiveElement, Styled, Subscription,
    Task, Window, div, prelude::FluentBuilder,
};

mod agents;
mod drawer;
mod events;
mod header;
mod keys;
mod order;
mod panel;
mod party;
pub mod restore;
mod update;
mod usage;
mod widths;

#[cfg(unix)]
pub use agents::Reports;
pub use drawer::Drawer;
pub use keys::{
    CloseSession, JumpToWaiting, LastSession, NewSession, NextSession, PreviousSession,
    SelectSession, ToggleDrawer, TogglePanel, ToggleParty, ToggleSessions, bind_keys,
};

use std::path::PathBuf;

use header::column_toggle;
use widths::{Dragged, Side};

use crate::{
    agents as characters,
    blur::Blur,
    files::Files,
    git::panel::{Acted, GitPanel},
    menus,
    sessions::{self, Session, orb},
    settings::{Settings, SettingsStore, StartIn, window::OpenSettings},
    term::{
        Events, Launch, Pty, TerminalView, config,
        shell::{ShellIntegration, launch},
        spawn,
    },
};

/// The key context of the main window's content.
pub const CONTEXT: &str = "Workspace";

/// The window's title while the program names none.
pub const TITLE: &str = "Ely Tea Party";

/// The main window's content: sessions, the terminal, and Git or Files, in Ely's columns.
pub struct Workspace {
    focus: FocusHandle,
    sessions: Vec<Session>,
    /// The session shown.
    active: usize,
    files: Entity<Files>,
    git: Entity<GitPanel>,
    /// What the window shows of the desktop, and the blur behind it.
    background: Blur,
    /// Column widths a drag shows before the settings hold them.
    dragged: Dragged,
    /// Draws the window at each step of the status marks while one shown moves.
    ticking: Option<Task<()>>,
    /// The drawer's height while its divider is dragged.
    drawer_dragged: Option<Pixels>,
    /// The tea party shows in place of the columns.
    party: bool,
    /// The sessions as last saved for the next start.
    saved: Option<restore::Saved>,
    party_focus: FocusHandle,
    /// The shown agents' subscription use.
    meters: usage::Meters,
    _settings: Subscription,
    _updates: Subscription,
    _acted: Subscription,
    _agents: [Task<()>; 3],
}

impl Workspace {
    /// The window's content with its first session `id`, `pty` started from `launch`.
    pub fn new(
        (id, pty, events): (u64, Pty, Events),
        launch: Launch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let git = cx.new(|cx| GitPanel::new(launch.cwd.clone(), window, cx));
        // A change the Git panel made shows in Files' marks too.
        let acted = cx.subscribe(&git, |this, _, _: &Acted, cx| {
            let folder = this.sessions[this.active].folder.clone();
            this.files.update(cx, |files, cx| files.show(&folder, cx));
        });
        let mut workspace = Self {
            focus: cx.focus_handle(),
            sessions: Vec::new(),
            active: 0,
            files: cx.new(|_| Files::new(launch.cwd.clone())),
            git,
            background: Blur::new(window, SettingsStore::get(cx)),
            dragged: Dragged::default(),
            ticking: None,
            drawer_dragged: None,
            party: false,
            saved: None,
            party_focus: cx.focus_handle(),
            meters: usage::Meters::default(),
            _settings: cx.observe_global_in::<SettingsStore>(window, |this, window, cx| {
                this.background.follow(window, SettingsStore::get(cx));
                cx.notify();
            }),
            _updates: cx.observe_global::<crate::update::Updates>(|_, cx| cx.notify()),
            _acted: acted,
            _agents: agents::watch(window, cx),
        };
        workspace.add((id, pty, events), launch, window, cx);
        workspace.select(0, window, cx);
        workspace
    }

    fn add(
        &mut self,
        (id, pty, events): (u64, Pty, Events),
        launch: Launch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let terminal = cx.new(|cx| TerminalView::new(pty, events, window, cx));
        let entity = terminal.entity_id();
        let events = cx.subscribe_in(&terminal, window, move |this, _, event, window, cx| {
            this.session_event(entity, event, window, cx)
        });
        let pid = terminal.read(cx).pid();
        log::info!("app: session {pid} opens in {}", launch.cwd.display());
        self.sessions.push(Session {
            id,
            agent: None,
            terminal,
            title: String::new(),
            folder: launch.cwd,
            reports: launch.reports,
            bell: false,
            waiting: false,
            look: None,
            drawer: None,
            pending: None,
            _events: events,
        });
    }

    /// Notes each session's mark `now`, and draws the window at each step while one shown moves.
    fn tick(&mut self, now: Instant, cx: &mut Context<Self>) {
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

    /// A session of the shell the settings name, in home or the shown session's folder.
    fn new_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        let id = characters::next_session();
        match start(folder, Some(id), cx) {
            Ok(((pty, events), launch)) => {
                self.add((id, pty, events), launch, window, cx);
                self.select(self.sessions.len() - 1, window, cx);
            }
            Err(error) => log::error!("app: no new session: {error:#}"),
        }
    }

    /// Shows session `index`, clears its bell and its waiting mark, and gives its terminal the keys.
    pub fn select(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.sessions.len() {
            return;
        }
        if index != self.active
            && let Some(hidden) = self.sessions.get(self.active)
        {
            let drawer = hidden.drawer.as_ref().map(|drawer| &drawer.terminal);
            for terminal in std::iter::once(&hidden.terminal).chain(drawer) {
                terminal.update(cx, |view, cx| view.let_go(cx));
            }
            hidden.terminal.update(cx, |view, _| view.set_front(false));
        }
        self.party = false;
        let session = &mut self.sessions[index];
        session.bell = false;
        session.waiting = false;
        session.terminal.update(cx, |view, _| view.set_front(true));
        window.focus(&session.terminal.focus_handle(cx), cx);
        let folder = session.folder.clone();
        self.files.update(cx, |files, cx| files.show(&folder, cx));
        self.git.update(cx, |git, cx| git.show(&folder, cx));
        self.active = index;
        log::debug!("app: session {} shown", index + 1);
        self.retitle(window);
        cx.notify();
    }

    /// Closes session `index`; its program hangs up. The last one closes the window.
    pub fn close(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.sessions.len() {
            return;
        }
        let session = self.sessions.remove(index);
        log::info!("app: session {} closes", session.terminal.read(cx).pid());
        if self.sessions.is_empty() {
            // Closed by hand, none comes back.
            self.keep();
            log::info!("app: no session left; the window closes");
            window.remove_window();
            return;
        }
        let shown = index == self.active;
        self.active = sessions::after_close(self.active, index, self.sessions.len());
        if shown {
            self.select(self.active, window, cx);
        } else {
            cx.notify();
        }
    }

    /// Its sessions, in their rows' order.
    pub fn sessions(&self) -> &[Session] {
        &self.sessions
    }

    /// The shown session's title, or the app's name while its program names none.
    fn title(&self) -> &str {
        self.sessions
            .get(self.active)
            .map(|session| session.title.as_str())
            .filter(|title| !title.is_empty())
            .unwrap_or(TITLE)
    }

    fn retitle(&self, window: &mut Window) {
        let title = self.title();
        if window.window_title() != title {
            log::debug!("app: the title is {title:?}");
            window.set_window_title(title);
        }
    }

    /// Shows or hides a side column, as the settings keep it.
    fn toggle(&mut self, flip: fn(&mut Settings), cx: &mut Context<Self>) {
        match SettingsStore::update(cx, flip) {
            Ok(()) => {
                let settings = SettingsStore::get(cx);
                let shown = |on: bool| if on { "shown" } else { "hidden" };
                log::info!(
                    "app: sessions {}, right column {}",
                    shown(settings.show_sessions),
                    shown(settings.show_panel)
                );
            }
            Err(error) => log::error!("app: the columns stay: {error:#}"),
        }
    }

    /// Shows the next session, or the one before, round the ends.
    fn step(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.sessions.len();
        let by = if forward { 1 } else { count - 1 };
        self.select((self.active + by) % count, window, cx);
    }
}

/// Starts the Shell setting in `folder`; a session's `agent` hooks reach the app under its id, a drawer's none.
fn start(folder: PathBuf, agent: Option<u64>, cx: &App) -> anyhow::Result<((Pty, Events), Launch)> {
    let settings = SettingsStore::get(cx);
    let mut launch = launch(&settings.shell, folder, &cx.global::<ShellIntegration>().0)?;
    if let Some(id) = agent {
        launch.env.extend(characters::env(id)?);
    }
    Ok((spawn(&launch, (80, 24), (8, 16), config(settings))?, launch))
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.background.fit(window);
        let now = Instant::now();
        self.tick(now, cx);
        let body = self.terminal_body(window, cx);
        let settings = SettingsStore::get(cx).clone();
        let workspace = cx.entity();
        let tip =
            |shown: bool, what: &str| format!("{} {what}", if shown { "Hide" } else { "Show" });
        // It leads the first column or ends the last.
        let left = column_toggle(
            &workspace,
            ("toggle-sessions", IconName::PanelLeft),
            tip(settings.show_sessions, "sessions"),
            |settings| settings.show_sessions = !settings.show_sessions,
        );
        let right = column_toggle(
            &workspace,
            ("toggle-panel", IconName::PanelRight),
            tip(settings.show_panel, "Git and Files"),
            |settings| settings.show_panel = !settings.show_panel,
        );
        // No menu bar off macOS: a gear opens Settings.
        let gear = (!cfg!(target_os = "macos")).then(|| {
            IconButton::new("open-settings", IconName::Settings)
                .variant(ButtonVariant::Ghost)
                .size(ControlSize::Sm)
                .tooltip("Settings")
                .on_click(|_, window, cx| window.dispatch_action(Box::new(OpenSettings), cx))
        });
        let end = div()
            .flex()
            .items_center()
            .gap_2()
            .children(update::update_button(cx))
            .children(gear)
            .child(right);
        let mut shell = ColumnShell::new("workspace").glass();
        let mut terminal = ShellColumn::new("terminal")
            .title(self.title().to_string())
            .child(body);
        if settings.show_sessions {
            let home = dirs::home_dir();
            let rows = self.sessions.iter().enumerate().map(|(index, session)| {
                let active = index == self.active;
                sessions::row(
                    &workspace,
                    index,
                    session,
                    active,
                    (home.as_deref(), now),
                    cx,
                )
            });
            let list = List::new().children(rows);
            let new = IconButton::new("new-session", IconName::Plus)
                .variant(ButtonVariant::Ghost)
                .size(ControlSize::Sm)
                .tooltip("New session")
                .on_click(move |_, window, cx| {
                    workspace.update(cx, |this, cx| this.new_session(window, cx))
                });
            shell = shell.column(
                self.side_column(Side::Sessions, cx)
                    .leading(left)
                    .action(party::party_button(cx.entity()))
                    .action(new)
                    .child(
                        div()
                            .id("sessions")
                            .size_full()
                            .overflow_y_scroll()
                            .p_2()
                            // A row dropped off the rows goes last.
                            .on_drop(cx.listener(|this, moving: &sessions::Moving, _, cx| {
                                this.move_session(moving.from, usize::MAX, cx)
                            }))
                            // A double click off the rows opens a session.
                            .on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
                                if event.click_count() == 2 {
                                    this.new_session(window, cx)
                                }
                            }))
                            .child(list),
                    ),
            );
        } else {
            terminal = terminal.leading(left);
        }
        if settings.show_panel {
            let tab = settings.panel;
            shell = shell.column(terminal).column(self.panel(tab, end, cx));
        } else {
            shell = shell.column(terminal.action(end));
        }
        // The context sits above the focus root, so its keys work wherever focus is.
        div()
            .size_full()
            .key_context(CONTEXT)
            // A press anywhere wakes the mascot; the terminals note the keys.
            .capture_any_mouse_down(|_, _, cx| crate::pet::touch(cx))
            .map(menus::window_actions)
            .on_action(cx.listener(|this, _: &ToggleSessions, _, cx| {
                this.toggle(
                    |settings| settings.show_sessions = !settings.show_sessions,
                    cx,
                )
            }))
            .on_action(cx.listener(|this, _: &TogglePanel, _, cx| {
                this.toggle(|settings| settings.show_panel = !settings.show_panel, cx)
            }))
            .on_action(cx.listener(|this, _: &NewSession, window, cx| this.new_session(window, cx)))
            .on_action(
                cx.listener(|this, _: &CloseSession, window, cx| this.close_focused(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &ToggleDrawer, window, cx| this.toggle_drawer(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &ToggleParty, window, cx| this.toggle_party(window, cx)),
            )
            .on_action(cx.listener(|this, _: &NextSession, window, cx| this.step(true, window, cx)))
            .on_action(
                cx.listener(|this, _: &PreviousSession, window, cx| this.step(false, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &JumpToWaiting, window, cx| this.jump_to_waiting(window, cx)),
            )
            .on_action(cx.listener(|this, _: &LastSession, window, cx| {
                this.select(this.sessions.len().saturating_sub(1), window, cx)
            }))
            .on_action(cx.listener(|this, select: &SelectSession, window, cx| {
                if select.0 < this.sessions.len() {
                    this.select(select.0, window, cx);
                } else {
                    cx.propagate();
                }
            }))
            .child(
                ResizeBorder::new()
                    .when(self.background.see_through(), ResizeBorder::glass)
                    .child(
                        FocusScope::new(&self.focus)
                            .root()
                            .size_full()
                            .child(if self.party {
                                self.party(cx).into_any_element()
                            } else {
                                shell.into_any_element()
                            }),
                    ),
            )
    }
}
