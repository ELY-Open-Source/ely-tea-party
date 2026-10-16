use ely_gpui_component::{
    buttons::{ButtonVariant, IconButton, SegmentedControl},
    lists::List,
    primitives::{FocusScope, IconName},
    shell::{ColumnShell, ResizeBorder, ShellColumn},
    theme::{ActiveTheme, ControlSize},
    typography::Caption,
};
use gpui::{
    App, AppContext, Context, Entity, EntityId, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, StyleRefinement,
    Styled, Subscription, Task, Window, div, prelude::FluentBuilder, rems,
};

mod agents;
mod keys;

#[cfg(unix)]
pub use agents::Reports;
pub use keys::{
    CloseSession, JumpToWaiting, LastSession, NewSession, NextSession, PreviousSession,
    SelectSession, TogglePanel, ToggleSessions, bind_keys,
};

use crate::{
    agents::{self as characters, Status},
    blur::Blur,
    files::Files,
    git::panel::{Acted, GitPanel},
    menus,
    sessions::{self, Session},
    settings::{PanelTab, Settings, SettingsStore, StartIn, window::OpenSettings},
    term::{
        Events, Launch, Pty, TerminalEvent, TerminalView, config,
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
    _settings: Subscription,
    _acted: Subscription,
    _agents: [Task<()>; 2],
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
            _settings: cx.observe_global_in::<SettingsStore>(window, |this, window, cx| {
                this.background.follow(window, SettingsStore::get(cx));
                cx.notify();
            }),
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
            _events: events,
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
        let started = launch(&settings.shell, folder, &cx.global::<ShellIntegration>().0).and_then(
            |mut launch| {
                launch.env.extend(characters::env(id)?);
                Ok((spawn(&launch, (80, 24), (8, 16), config(settings))?, launch))
            },
        );
        match started {
            Ok(((pty, events), launch)) => {
                self.add((id, pty, events), launch, window, cx);
                self.select(self.sessions.len() - 1, window, cx);
            }
            Err(error) => log::error!("app: no new session: {error:#}"),
        }
    }

    /// Shows session `index`, clears its bell, and gives its terminal the keys.
    pub fn select(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.sessions.len() {
            return;
        }
        if index != self.active
            && let Some(hidden) = self.sessions.get(self.active)
        {
            hidden.terminal.update(cx, |view, cx| view.let_go(cx));
        }
        let session = &mut self.sessions[index];
        session.bell = false;
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

    fn session_event(
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
            TerminalEvent::Title(title) => session.title.clone_from(title),
            TerminalEvent::Folder(folder) => {
                session.folder.clone_from(folder);
                session.reports = true;
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

/// Shows Git or Files in the right column, as the settings keep it.
fn show_tab(value: &SharedString, cx: &mut App) {
    let panel = match value.as_ref() {
        "git" => PanelTab::Git,
        "files" => PanelTab::Files,
        other => unreachable!("app: no {other} tab"),
    };
    if SettingsStore::get(cx).panel == panel {
        return;
    }
    match SettingsStore::update(cx, |settings| settings.panel = panel) {
        Ok(()) => log::info!("app: the right column shows {panel:?}"),
        Err(error) => log::error!("app: the right column stays: {error:#}"),
    }
}

/// A header button that hides or shows a column, saved.
fn column_toggle(
    workspace: &Entity<Workspace>,
    (id, icon): (&'static str, IconName),
    tip: String,
    flip: fn(&mut Settings),
) -> IconButton {
    let workspace = workspace.clone();
    IconButton::new(id, icon)
        .variant(ButtonVariant::Ghost)
        .size(ControlSize::Sm)
        .tooltip(tip)
        .on_click(move |_, _, cx| workspace.update(cx, |this, cx| this.toggle(flip, cx)))
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.background.fit(window);
        let settings = SettingsStore::get(cx);
        let width = cx.theme().sidebar_width(false);
        // Wider than Ely's sidebar: the characters need the room.
        let sessions_width = rems(18.);
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
            .children(gear)
            .child(right);
        let mut shell = ColumnShell::new("workspace").glass();
        let shown = self
            .sessions
            .get(self.active)
            .map(|session| session.terminal.clone());
        // Cached: the characters' frames redraw the window, not the grid.
        let fill = || StyleRefinement::default().size_full();
        let mut terminal = ShellColumn::new("terminal")
            .title(self.title().to_string())
            .children(shown.map(|terminal| terminal.cached(fill())));
        if settings.show_sessions {
            let home = dirs::home_dir();
            let now = std::time::Instant::now();
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
                ShellColumn::new("sessions")
                    .width(sessions_width)
                    .leading(left)
                    .action(new)
                    .child(
                        div()
                            .id("sessions")
                            .size_full()
                            .overflow_y_scroll()
                            .p_2()
                            .child(list),
                    ),
            );
        } else {
            terminal = terminal.leading(left);
        }
        if settings.show_panel {
            let tab = settings.panel;
            let chosen = match tab {
                PanelTab::Git => "git",
                PanelTab::Files => "files",
            };
            let switch = SegmentedControl::new("panel-tab", chosen)
                .segment("git", "Git", None)
                .segment("files", "Files", None)
                .size(ControlSize::Sm)
                .on_change(|value, _, cx| show_tab(value, cx));
            let quiet = self
                .sessions
                .get(self.active)
                .is_some_and(|session| !session.reports);
            let caption = || Caption::new("This shell reports no folder.").px_3().pt_2();
            // Hidden, not dropped, so each keeps its state.
            let body = |on: bool| {
                div()
                    .flex_1()
                    .min_h_0()
                    .p_2()
                    .when(!on, |body| body.hidden())
            };
            // Git's list draws every row, so terminal output must not redraw it.
            shell = shell.column(terminal).column(
                ShellColumn::new("panel")
                    .width(width)
                    .action(end)
                    // Below the title bar, clear of Windows' caption buttons.
                    .child(div().flex().justify_center().px_2().pt_2().child(switch))
                    .children(quiet.then(caption))
                    .child(body(tab == PanelTab::Git).child(self.git.clone().cached(fill())))
                    .child(body(tab == PanelTab::Files).child(self.files.clone().cached(fill()))),
            );
        } else {
            shell = shell.column(terminal.action(end));
        }
        // The context sits above the focus root, so its keys work wherever focus is.
        div()
            .size_full()
            .key_context(CONTEXT)
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
                cx.listener(|this, _: &CloseSession, window, cx| {
                    this.close(this.active, window, cx)
                }),
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
                    .child(FocusScope::new(&self.focus).root().size_full().child(shell)),
            )
    }
}
