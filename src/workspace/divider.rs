use ely_gpui_component::{layout::resize_edge, shell::ColumnShell, theme::ActiveTheme};
use gpui::{
    App, AppContext, Axis, Context, CursorStyle, Div, DragMoveEvent, EmptyView, InteractiveElement,
    MouseButton, ParentElement, Pixels, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px,
};

use super::Workspace;
use crate::settings::{SESSIONS_WIDTH, SettingsStore};

/// The sessions column's divider, as gpui drags it.
pub struct Divider;

/// The least the terminal keeps beside the columns while the divider moves.
const TERMINAL_ROOM: f32 = 240.0;

impl Workspace {
    /// The sessions column's width: the drag's while one runs, else the saved one.
    pub(super) fn sessions_width(&self, cx: &App) -> Pixels {
        self.dragged
            .unwrap_or_else(|| px(SettingsStore::get(cx).sessions_width))
    }

    /// Moves the divider to `event`'s pointer, within its range and the terminal's room.
    fn drag_to(&mut self, event: &DragMoveEvent<Divider>, window: &Window, cx: &mut Context<Self>) {
        let settings = SettingsStore::get(cx);
        let panel = if settings.show_panel {
            cx.theme().sidebar_width(false).to_pixels(window.rem_size())
        } else {
            px(0.)
        };
        let room = f32::from(event.bounds.size.width - panel) - TERMINAL_ROOM;
        let (least, most) = (*SESSIONS_WIDTH.start(), *SESSIONS_WIDTH.end());
        let wanted = f32::from(event.event.position.x - event.bounds.origin.x);
        let width = px(wanted.clamp(least, most.min(room).max(least)).round());
        if self.dragged != Some(width) {
            self.dragged = Some(width);
            cx.notify();
        }
    }

    /// Saves the dragged width once the button comes up.
    fn keep_width(&mut self, cx: &mut Context<Self>) {
        let Some(width) = self.dragged.take() else {
            return;
        };
        let width = f32::from(width);
        match SettingsStore::update(cx, |settings| settings.sessions_width = width) {
            Ok(()) => log::info!("app: the sessions column is {width} px wide"),
            Err(error) => log::error!("app: the sessions column's width stays: {error:#}"),
        }
        cx.notify();
    }

    /// `shell` under the sessions column's divider, when `shown`.
    pub(super) fn wrap(
        &self,
        shell: ColumnShell,
        shown: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let half = cx.theme().handle_hit().to_pixels(window.rem_size()) / 2.;
        let handle = resize_edge("sessions-divider", Axis::Horizontal, cx)
            .absolute()
            .top_0()
            .left(self.sessions_width(cx) - half)
            .occlude()
            .on_drag(Divider, |_, _, _, cx| cx.new(|_| EmptyView))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.keep_width(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.keep_width(cx)),
            );
        // Over everything while dragging, so the pointer keeps its shape.
        let cover = || {
            div()
                .absolute()
                .inset_0()
                .cursor(CursorStyle::ResizeLeftRight)
        };
        div()
            .relative()
            .size_full()
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<Divider>, window, cx| {
                    this.drag_to(event, window, cx)
                }),
            )
            .child(shell)
            .when(shown, |root| root.child(handle))
            .when(self.dragged.is_some(), |root| root.child(cover()))
    }
}
