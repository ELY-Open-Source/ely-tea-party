//! Each session's drawer: one shell of the user's own under the session's terminal, in its folder, on Cmd-J.

use std::path::PathBuf;

use ely_gpui_component::{
    buttons::{ButtonVariant, IconButton},
    layout::resize_edge,
    primitives::IconName,
    theme::{ActiveTheme, ControlSize},
    typography::Caption,
};
use gpui::{
    AppContext, Axis, Context, Div, DragMoveEvent, EmptyView, Entity, Focusable,
    InteractiveElement, MouseButton, ParentElement, Pixels, StatefulInteractiveElement,
    StyleRefinement, Styled, Subscription, Window, div, prelude::FluentBuilder, px,
};

use super::Workspace;
use crate::{
    agents::detect::{Foreground, foreground},
    sessions,
    settings::{DRAWER_HEIGHT, SettingsStore},
    term::{TerminalEvent, TerminalView},
};

/// The least the session's own terminal keeps above an open drawer.
const TERMINAL_ROOM: f32 = 120.0;

/// A session's drawer: its shell, whether it shows, and what it last named.
pub struct Drawer {
    pub terminal: Entity<TerminalView>,
    /// It shows under the session's terminal.
    pub open: bool,
    /// A job other than its shell held its keys at the last tick.
    pub busy: bool,
    title: String,
    folder: PathBuf,
    _events: Subscription,
}

impl Drawer {
    /// Reads whether a job holds its keys; returns whether that changed.
    pub fn check(&mut self, cx: &gpui::App) -> bool {
        let view = self.terminal.read(cx);
        let busy = foreground(view.pid(), view.foreground()) != Foreground::Shell;
        std::mem::replace(&mut self.busy, busy) != busy
    }
}

/// The drawer's divider, as gpui drags it.
pub struct DrawerDivider;

impl Workspace {
    /// Cmd-J: shows the shown session's drawer and gives it the keys; with the keys there already, hides it.
    pub(super) fn toggle_drawer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self.sessions.get_mut(self.active) else {
            return;
        };
        let id = session.id;
        match session.drawer.as_mut() {
            Some(drawer)
                if drawer.open
                    && drawer
                        .terminal
                        .focus_handle(cx)
                        .contains_focused(window, cx) =>
            {
                drawer.open = false;
                drawer.terminal.update(cx, |view, cx| view.let_go(cx));
                window.focus(&session.terminal.focus_handle(cx), cx);
                log::debug!("app: session {id}'s drawer hides");
            }
            Some(drawer) => {
                drawer.open = true;
                window.focus(&drawer.terminal.focus_handle(cx), cx);
                log::debug!("app: session {id}'s drawer shows");
            }
            None => {
                let folder = session.folder.clone();
                match super::start(folder.clone(), None, cx) {
                    Ok(((pty, events), launch)) => {
                        let terminal = cx.new(|cx| TerminalView::new(pty, events, window, cx));
                        let events = cx.subscribe_in(
                            &terminal,
                            window,
                            move |this, _, event, window, cx| {
                                this.drawer_event(id, event, window, cx)
                            },
                        );
                        log::info!(
                            "app: session {id}'s drawer {} opens in {}",
                            terminal.read(cx).pid(),
                            launch.cwd.display()
                        );
                        window.focus(&terminal.focus_handle(cx), cx);
                        let session = &mut self.sessions[self.active];
                        session.drawer = Some(Drawer {
                            terminal,
                            open: true,
                            busy: false,
                            title: String::new(),
                            folder,
                            _events: events,
                        });
                    }
                    Err(error) => log::error!("app: no drawer for session {id}: {error:#}"),
                }
            }
        }
        cx.notify();
    }

    /// Cmd-W: closes the drawer that holds the keys, else the shown session.
    pub(super) fn close_focused(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focused = self.sessions.get(self.active).is_some_and(|session| {
            session.drawer.as_ref().is_some_and(|drawer| {
                drawer.open
                    && drawer
                        .terminal
                        .focus_handle(cx)
                        .contains_focused(window, cx)
            })
        });
        if focused {
            self.close_drawer(self.active, window, cx);
        } else {
            self.close(self.active, window, cx);
        }
    }

    /// Closes session `index`'s drawer; its shell hangs up, and the session's terminal takes the keys if it showed.
    fn close_drawer(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let session = &mut self.sessions[index];
        let Some(drawer) = session.drawer.take() else {
            return;
        };
        log::info!(
            "app: session {}'s drawer {} closes",
            session.id,
            drawer.terminal.read(cx).pid()
        );
        if index == self.active && drawer.open {
            window.focus(&session.terminal.focus_handle(cx), cx);
        }
        cx.notify();
    }

    fn drawer_event(
        &mut self,
        id: u64,
        event: &TerminalEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.sessions.iter().position(|session| session.id == id) else {
            return;
        };
        let shown = index == self.active;
        let session = &mut self.sessions[index];
        let Some(drawer) = session.drawer.as_mut() else {
            return;
        };
        match event {
            TerminalEvent::Title(title) => drawer.title.clone_from(title),
            TerminalEvent::Folder(folder) => drawer.folder.clone_from(folder),
            TerminalEvent::Bell if !(session.bell || shown && drawer.open) => {
                log::debug!("app: a bell in session {id}'s hidden drawer");
                session.bell = true;
            }
            TerminalEvent::Bell | TerminalEvent::Shows(_) => {}
            TerminalEvent::Exited(status) => {
                log::info!("app: session {id}'s drawer ended, {status}");
                self.close_drawer(index, window, cx);
                return;
            }
        }
        cx.notify();
    }

    /// The drawer's height: the drag's while one runs, else the saved one.
    fn drawer_height(&self, cx: &gpui::App) -> Pixels {
        self.drawer_dragged
            .unwrap_or_else(|| px(SettingsStore::get(cx).drawer_height))
    }

    fn drag_drawer(&mut self, event: &DragMoveEvent<DrawerDivider>, cx: &mut Context<Self>) {
        let bounds = event.bounds;
        let room = f32::from(bounds.size.height) - TERMINAL_ROOM;
        let (least, most) = (*DRAWER_HEIGHT.start(), *DRAWER_HEIGHT.end());
        let wanted = f32::from(bounds.origin.y + bounds.size.height - event.event.position.y);
        let height = px(wanted.clamp(least, most.min(room).max(least)).round());
        if self.drawer_dragged != Some(height) {
            self.drawer_dragged = Some(height);
            cx.notify();
        }
    }

    /// Saves the dragged height once the button comes up.
    fn keep_drawer_height(&mut self, cx: &mut Context<Self>) {
        let Some(height) = self.drawer_dragged.take() else {
            return;
        };
        let height = f32::from(height);
        match SettingsStore::update(cx, |settings| settings.drawer_height = height) {
            Ok(()) => log::info!("app: the drawer is {height} px high"),
            Err(error) => log::error!("app: the drawer's height stays: {error:#}"),
        }
        cx.notify();
    }

    /// The terminal column's body: the shown session's terminal, and its drawer under it when open.
    pub(super) fn terminal_body(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        // Cached: the characters' frames redraw the window, not the grids.
        let fill = || StyleRefinement::default().size_full();
        let session = self.sessions.get(self.active);
        let main = div()
            .flex_1()
            .min_h_0()
            .children(session.map(|session| session.terminal.clone().cached(fill())));
        let drawer = session
            .and_then(|session| session.drawer.as_ref())
            .filter(|drawer| drawer.open);
        let Some(drawer) = drawer else {
            return div().size_full().flex().flex_col().child(main);
        };
        let home = dirs::home_dir();
        let name = if drawer.title.is_empty() {
            sessions::shown(&drawer.folder, home.as_deref())
        } else {
            sessions::one_line(&drawer.title)
        };
        let workspace = cx.entity();
        let close = IconButton::new("close-drawer", IconName::X)
            .variant(ButtonVariant::Ghost)
            .size(ControlSize::Sm)
            .tooltip("Close drawer")
            .on_click(move |_, window, cx| {
                workspace.update(cx, |this, cx| this.close_drawer(this.active, window, cx))
            });
        let header = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .px_3()
            .h(cx.theme().control_height(ControlSize::Sm))
            .child(Caption::new(name))
            .child(close);
        let half = cx.theme().handle_hit().to_pixels(window.rem_size()) / 2.;
        let handle = resize_edge("drawer-divider", Axis::Vertical, cx)
            .absolute()
            .top(-half)
            .occlude()
            .on_drag(DrawerDivider, |_, _, _, cx| cx.new(|_| EmptyView))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.keep_drawer_height(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.keep_drawer_height(cx)),
            );
        // Over everything while dragging, so the pointer keeps its shape.
        let cover = || {
            div()
                .absolute()
                .inset_0()
                .cursor(gpui::CursorStyle::ResizeUpDown)
        };
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<DrawerDivider>, _, cx| {
                    this.drag_drawer(event, cx)
                }),
            )
            .child(main)
            .child(
                div()
                    .relative()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .h(self.drawer_height(cx))
                    .border_t_1()
                    .border_color(cx.theme().colors.border)
                    .child(handle)
                    .child(header)
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .child(drawer.terminal.clone().cached(fill())),
                    ),
            )
            .when(self.drawer_dragged.is_some(), |body| body.child(cover()))
    }
}
