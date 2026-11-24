use std::time::Instant;

use ely_gpui_component::{
    buttons::{ButtonVariant, IconButton},
    i18n::text,
    primitives::{FocusScope, IconName},
    shell::{ColumnShell, ResizeBorder, ShellColumn},
    theme::{ActiveTheme, ControlSize},
};
use gpui::{
    AppContext, ClickEvent, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Pixels, Render, StatefulInteractiveElement, Styled, Subscription,
    Task, Window, div, prelude::FluentBuilder,
};

mod agents;
mod butler;
mod companion;
mod divider;
mod drawer;
mod drawer_share;
mod events;
mod header;
mod keys;
mod order;
mod panel;
mod party;
mod rename;
pub mod restore;
mod session;
mod sidebar;
mod speed;
mod switcher;
mod update;
mod usage;
mod widths;

#[cfg(unix)]
pub use agents::Reports;
pub use drawer::Drawer;
pub use keys::{
    CloseSession, JumpToWaiting, LastSession, MovePane, NewSession, NextPane, NextSession,
    PreviousPane, PreviousSession, QuickSwitch, SelectSession, SplitDown, SplitRight, ToggleDrawer,
    TogglePanel, ToggleParty, ToggleSessions, bind_keys,
};

use header::column_toggle;
use widths::{Dragged, Side};

use crate::{
    blur::Blur,
    files::{Files, Here},
    git::panel::{Acted, GitPanel},
    menus,
    sessions::{self, Program, Session},
    settings::{PanelTab, SettingsStore, window::OpenSettings},
    term::{Events, Launch, Pty},
};

mod control;
mod folder;
mod groups;
mod leave;
mod panes;
mod wallpaper;

pub use leave::Leave;

/// The key context of the main window's content.
pub const CONTEXT: &str = "Workspace";

/// The window's title while the program names none.
pub const TITLE: &str = "Ely Tea Party";

/// The main window's content: sessions, the terminal, and Git or Files, in Ely's columns.
pub struct Workspace {
    focus: FocusHandle,
    sessions: Vec<Session>,
    sidebar: sidebar::Sidebar,
    switcher: Option<Entity<switcher::Switcher>>,
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
    butler: crate::butler::Briefing,
    companion: companion::Companion,
    /// The sessions as last saved for the next start.
    saved: Option<restore::Saved>,
    steps: restore::steps::Keeper,
    party_focus: FocusHandle,
    /// The shown agents' subscription use.
    meters: usage::Meters,
    /// The right column's lit tab, gliding between tabs.
    glide: panel::Glide,
    /// The shells this system holds, found as the window opened.
    found: Vec<crate::profiles::Found>,
    choosing_folder: bool,
    folder_actions: bool,
    /// A close that waits for the user's word, and the programs it would end.
    leaving: Option<(Leave, Vec<String>)>,
    /// The sessions' groups, in the order their rows show.
    groups: Vec<groups::Group>,
    _settings: Subscription,
    _updates: Subscription,
    _acted: Subscription,
    _agents: [Task<()>; 2],
    _here: Subscription,
}

impl Workspace {
    /// The window's content with its first session `id`, `pty` started from `launch` to run `program`.
    pub fn new(
        id: u64,
        (pty, events, launch): (Pty, Events, Launch),
        program: Program,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let git = cx.new(|cx| GitPanel::new(launch.cwd.clone(), window, cx));
        // A change the Git panel made shows in Files' marks too.
        let acted = cx.subscribe(&git, |this, _, _: &Acted, cx| {
            let folder = this.sessions[this.active].pane().folder.clone();
            this.files.update(cx, |files, cx| files.show(&folder, cx));
        });
        let files = cx.new(|cx| {
            let mut files = Files::new(launch.cwd.clone());
            files.poll(cx);
            files
        });
        let here = cx.subscribe_in(&files, window, |this, _, Here(folder), window, cx| {
            if let Err(error) = this.here(folder.clone(), window, cx) {
                folder::failed(error, window, cx);
            }
        });
        let mut workspace = Self {
            focus: cx.focus_handle(),
            sessions: Vec::new(),
            sidebar: sidebar::Sidebar::default(),
            switcher: None,
            active: 0,
            files,
            git,
            background: Blur::new(window, SettingsStore::get(cx)),
            dragged: Dragged::default(),
            ticking: None,
            found: crate::profiles::detected(),
            choosing_folder: false,
            folder_actions: false,
            drawer_dragged: None,
            party: false,
            butler: crate::butler::Briefing::default(),
            companion: companion::Companion::default(),
            saved: None,
            steps: Default::default(),
            party_focus: cx.focus_handle(),
            meters: usage::Meters::default(),
            glide: panel::Glide::default(),
            leaving: None,
            groups: Vec::new(),
            _settings: cx.observe_global_in::<SettingsStore>(window, |this, window, cx| {
                this.background.follow(window, SettingsStore::get(cx));
                this.quiet_companion(cx);
                let settings = SettingsStore::get(cx);
                if !settings.files_menu || !settings.show_panel || settings.panel != PanelTab::Files
                {
                    this.folder_actions = false;
                }
                crate::memory::organize::cancel(cx);
                if !SettingsStore::get(cx).butler.enabled {
                    this.butler.cancel();
                    this.butler.text.clear();
                    this.butler.error = None;
                }
                cx.notify();
            }),
            _updates: cx.observe_global::<crate::update::Updates>(|_, cx| cx.notify()),
            _acted: acted,
            _agents: agents::watch(window, cx),
            _here: here,
        };
        workspace.watch_companion(window, cx);
        workspace.add(id, (pty, events, launch), program, window, cx);
        workspace.select(0, window, cx);
        workspace
    }

    /// Shows session `index`, clears its bell and its waiting mark, and gives its terminal the keys.
    pub fn select(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.sessions.len() {
            return;
        }
        let session = &self.sessions[index];
        self.sidebar.reveal(session.group, &session.main().folder);
        if self.switcher.take().is_some() {
            log::debug!("switcher: closed as a session takes focus");
        }
        if index != self.active {
            self.quiet_companion(cx);
        }
        if index != self.active
            && let Some(hidden) = self.sessions.get(self.active)
        {
            let drawer = hidden.drawer.as_ref().map(|drawer| &drawer.terminal);
            for terminal in hidden.panes.iter().map(|pane| &pane.terminal).chain(drawer) {
                terminal.update(cx, |view, cx| view.let_go(cx));
            }
            for pane in &hidden.panes {
                pane.terminal
                    .update(cx, |view, _| view.set_front(false, window));
            }
        }
        self.party = false;
        let session = &mut self.sessions[index];
        switcher::visit(session.id, cx);
        session.bell = false;
        session.waiting = false;
        for pane in &session.panes {
            pane.terminal
                .update(cx, |view, _| view.set_front(true, window));
        }
        window.focus(&session.terminal().focus_handle(cx), cx);
        self.active = index;
        self.show_folder(cx);
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
        log::info!("app: session {} closes", index + 1);
        // Its textures leave the window's atlas with it.
        let drawer = session.drawer.as_ref().map(|drawer| &drawer.terminal);
        for terminal in session
            .panes
            .iter()
            .map(|pane| &pane.terminal)
            .chain(drawer)
        {
            terminal.update(cx, |view, _| view.set_front(false, window));
        }
        if self.sessions.is_empty() {
            // Closed by hand, none comes back.
            self.keep();
            log::info!("app: no session left; the window closes");
            window.remove_window();
            return;
        }
        let shown = index == self.active;
        self.active = sessions::after_close(self.active, index, self.sessions.len());
        self.prune_groups(window, cx);
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

    /// What the shown session is called.
    fn title(&self) -> &str {
        self.sessions.get(self.active).map_or(TITLE, Session::named)
    }

    fn retitle(&self, window: &mut Window) {
        let title = self.title();
        if window.window_title() != title {
            log::debug!("app: the title is {title:?}");
            window.set_window_title(title);
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.background.fit(window);
        let now = Instant::now();
        self.tick(now, cx);
        let body = self.terminal_body(window, cx);
        let bubble = self.butler_bubble(window, cx);
        let settings = SettingsStore::get(cx).clone();
        let base = cx.theme().colors.bg;
        let workspace = cx.entity();
        // It leads the first column or ends the last.
        let left = column_toggle(
            &workspace,
            ("toggle-sessions", IconName::PanelLeft),
            (
                settings.show_sessions,
                ["et.tip.hide_sessions", "et.tip.show_sessions"],
            ),
            |settings| settings.show_sessions = !settings.show_sessions,
            cx,
        );
        let right = column_toggle(
            &workspace,
            ("toggle-panel", IconName::PanelRight),
            (
                settings.show_panel,
                ["et.tip.hide_panel", "et.tip.show_panel"],
            ),
            |settings| settings.show_panel = !settings.show_panel,
            cx,
        );
        // No menu bar off macOS: a gear opens Settings. A narrow header clips its end, so the gear goes first.
        let gear = (!cfg!(target_os = "macos")).then(|| {
            IconButton::new("open-settings", IconName::Settings)
                .variant(ButtonVariant::Ghost)
                .size(ControlSize::Sm)
                .tooltip(text(cx, "et.settings.title", &[]))
                .on_click(|_, window, cx| window.dispatch_action(Box::new(OpenSettings), cx))
        });
        let end = div()
            .flex()
            .items_center()
            .gap_2()
            .children(update::update_button(cx))
            .children(gear)
            .child(right);
        let this = cx.entity();
        let mut shell = ColumnShell::new("workspace")
            .glass()
            .on_close(move |window, cx| {
                if !this.update(cx, |this, cx| this.asks(Leave::Window, cx)) {
                    this.update(cx, |this, _| this.keep());
                    window.remove_window();
                }
            });
        let mut terminal = ShellColumn::new("terminal")
            .title(self.title().to_string())
            .child(
                div()
                    .relative()
                    .size_full()
                    .child(body)
                    .child(wallpaper::ruler()),
            );
        if settings.show_sessions {
            let list = self.session_list(now, cx);
            let new = IconButton::new("new-session", IconName::Plus)
                .variant(ButtonVariant::Ghost)
                .size(ControlSize::Sm)
                .tooltip(text(cx, "et.session.new", &[]))
                .on_click(move |_, window, cx| {
                    workspace.update(cx, |this, cx| this.new_session(window, cx))
                });
            let column = self
                .side_column(Side::Sessions, cx)
                .leading(left)
                .action(party::party_button(cx.entity()))
                .action(new)
                .action(crate::profiles::button(&cx.entity(), &self.found, cx));
            shell = shell.column(
                column
                    .child(
                        div()
                            .id("sessions")
                            .size_full()
                            .overflow_y_scroll()
                            .p_2()
                            // A row dropped off the rows goes last.
                            .on_drop(cx.listener(|this, moving: &sessions::Moving, _, cx| {
                                this.move_sidebar_session(moving.from, usize::MAX, cx)
                            }))
                            // A double click off the rows opens a session.
                            .on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
                                if event.click_count() == 2 {
                                    this.new_session(window, cx)
                                }
                            }))
                            .child(list),
                    )
                    .child(divider::sidebar(Side::Sessions, cx)),
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
        crate::interface::dress(div(), cx)
            .size_full()
            .relative()
            .key_context(CONTEXT)
            // A press anywhere wakes the mascot; the terminals note the keys.
            .capture_any_mouse_down(|_, _, cx| crate::pet::touch(cx))
            .on_scroll_wheel(|_, _, cx| crate::pet::touch(cx))
            .capture_key_down(|_, _, cx| crate::pet::touch(cx))
            .map(menus::window_actions)
            .on_action(
                cx.listener(|this, _: &QuickSwitch, window, cx| this.toggle_switcher(window, cx)),
            )
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
            .on_action(cx.listener(|this, _: &SplitRight, window, cx| {
                this.split(gpui::Axis::Horizontal, window, cx)
            }))
            .on_action(cx.listener(|this, _: &SplitDown, window, cx| {
                this.split(gpui::Axis::Vertical, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &NextPane, window, cx| this.step_pane(true, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &PreviousPane, window, cx| this.step_pane(false, window, cx)),
            )
            .on_action(
                cx.listener(|this, MovePane(toward): &MovePane, _, cx| this.move_pane(*toward, cx)),
            )
            .on_action(cx.listener(|this, _: &LastSession, window, cx| {
                if let Some(&index) = this.session_order(cx).last() {
                    this.select(index, window, cx);
                }
            }))
            .on_action(cx.listener(|this, select: &SelectSession, window, cx| {
                if let Some(&index) = this.session_order(cx).get(select.0) {
                    this.select(index, window, cx);
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
                                div()
                                    .relative()
                                    .size_full()
                                    .children(wallpaper::paint(&settings, base, window, cx))
                                    .child(shell)
                                    .children(self.leave_prompt(cx))
                                    .into_any_element()
                            }),
                    ),
            )
            .children(bubble)
            .children(self.companion_bubble(window, cx))
            .child(crate::butler::outside_click_listener())
            .children(self.switcher.clone())
    }
}
