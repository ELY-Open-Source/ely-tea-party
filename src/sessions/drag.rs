//! Rows drag up and down the sessions column to reorder the sessions.

use std::{path::Path, time::Instant};

use ely_gpui_component::{
    menus::ContextMenu,
    primitives::Tooltip,
    theme::{ActiveTheme, Radius},
};
use gpui::{
    App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Window, div, transparent_black,
};

use super::{Session, TITLE, item, menu, one_line, shown};
use crate::workspace::Workspace;

/// The session shown once session `from` moved to `to`, `active` having been shown.
pub fn after_move(active: usize, from: usize, to: usize) -> usize {
    match active {
        _ if active == from => to,
        _ if from < active && active <= to => active - 1,
        _ if to <= active && active < from => active + 1,
        _ => active,
    }
}

/// A session's row on its way to another place.
pub struct Moving {
    pub from: usize,
    title: SharedString,
}

/// What follows the pointer while a row moves: its title.
struct Ghost(SharedString);

impl Render for Ghost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .px_3()
            .py_1()
            .rounded(theme.radius(Radius::Md))
            .bg(theme.colors.overlay)
            .border_1()
            .border_color(theme.colors.border)
            .text_color(theme.colors.fg)
            .text_sm()
            .child(self.0.clone())
    }
}

/// Session `index`'s row, which drags to another session's place; a right press opens its menu.
pub fn row(
    workspace: &Entity<Workspace>,
    index: usize,
    session: &Session,
    active: bool,
    when: (Option<&Path>, Instant),
    cx: &App,
) -> impl IntoElement {
    let pid = session.terminal.read(cx).pid() as usize;
    let title = match &session.agent {
        Some(agent) => session.called(agent),
        None if session.title.is_empty() => TITLE.to_string(),
        None => one_line(&session.title),
    };
    let full = shown(&session.folder, when.0);
    let moving = Moving {
        from: index,
        title: title.into(),
    };
    let workspace = workspace.clone();
    let item = item(&workspace, index, session, active, when.0, when.1, cx);
    let menu = menu::row(&workspace, session.terminal.entity_id());
    div()
        .id(("session-row", pid))
        .rounded(cx.theme().radius(Radius::Lg))
        .border_1()
        .border_color(transparent_black())
        .tooltip(Tooltip::text(full))
        .on_drag(moving, |moving, _, _, cx| {
            cx.new(|_| Ghost(moving.title.clone()))
        })
        .drag_over::<Moving>(move |style, moving, _, cx| {
            if moving.from == index {
                style
            } else {
                style.border_color(cx.theme().colors.focus)
            }
        })
        .on_drop(move |moving: &Moving, _, cx| {
            workspace.update(cx, |this, cx| this.move_session(moving.from, index, cx))
        })
        .child(
            ContextMenu::new(("session-menu", pid), menu)
                .w_full()
                .child(item),
        )
}
