use ely_gpui_component::{
    buttons::{ButtonVariant, IconButton},
    i18n::text,
    primitives::{Icon, IconName, Tooltip},
    theme::{ActiveTheme, ControlSize, IconSize, Radius, TextSize},
    typography::Ellipsis,
};
use gpui::{App, Entity, FontWeight, MouseButton, Role, div, prelude::*};

use super::{Session, editor, one_line};
use crate::{agents::icons, workspace::Workspace};

pub(crate) fn title(session: &Session) -> String {
    let name = if session.name.is_empty() && session.title.is_empty() {
        session
            .agent
            .as_ref()
            .map_or(session.named(), |agent| icons::name(agent.kind))
    } else {
        session.named()
    };
    one_line(name)
}

pub(super) fn item(
    workspace: &Entity<Workspace>,
    index: usize,
    session: &Session,
    active: bool,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let state = session.task_state(cx);
    let status = text(cx, state.label(), &[]);
    let editing = session.editing.is_some();
    let select = workspace.clone();
    let result = workspace.clone();
    let link = session.link_id;
    let kind = session.agent.as_ref().map(|agent| agent.kind);
    let heading = match &session.editing {
        Some((input, _)) => editor(workspace, session, input).into_any_element(),
        None => Ellipsis::new(title(session)).into_any_element(),
    };
    let running = session
        .drawer
        .as_ref()
        .is_some_and(|drawer| drawer.busy && !(active && drawer.open));
    div()
        .id(("compact-session", session.id))
        .role(Role::ListItem)
        .aria_label(title(session))
        .aria_description(status.clone())
        .aria_selected(active)
        .min_h(theme.control_height(ControlSize::Md))
        .px_2()
        .py_1()
        .pr_8()
        .flex()
        .items_center()
        .gap_2()
        .rounded(theme.radius(Radius::Md))
        .text_size(theme.text_size(TextSize::Sm))
        .text_color(theme.colors.fg)
        .when(active, |row| row.bg(theme.colors.active))
        .when(!active, |row| {
            row.hover(|style| style.bg(theme.colors.hover))
        })
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
        .on_click(move |event, window, cx| {
            cx.stop_propagation();
            match (editing, event.click_count()) {
                (true, _) => {}
                (false, 2..) => select.update(cx, |this, cx| this.rename(index, window, cx)),
                (false, _) => select.update(cx, |this, cx| this.select(index, window, cx)),
            }
        })
        .child(div().flex_none().child(icons::icon(kind, cx)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .when(session.bell || state.needs_attention(), |title| {
                    title.font_weight(FontWeight::SEMIBOLD)
                })
                .child(heading),
        )
        .children(running.then(|| {
            Icon::new(IconName::Terminal)
                .size(IconSize::Xs)
                .color(theme.colors.fg_muted)
        }))
        .children(session.log.completion().map(|_| {
            IconButton::new(("session-result", session.id), IconName::BookOpen)
                .variant(ButtonVariant::Ghost)
                .size(ControlSize::Sm)
                .tooltip(text(cx, "et.task.result", &[]))
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    result.update(cx, |this, cx| this.open_task_result(link, window, cx));
                })
        }))
        .child(
            div()
                .id(("compact-status", session.id))
                .flex_none()
                .tooltip(Tooltip::text(status))
                .child(
                    Icon::new(if session.bell {
                        IconName::Bell
                    } else {
                        IconName::CircleDot
                    })
                    .size(IconSize::Xs)
                    .color(if session.bell {
                        theme.colors.warning
                    } else {
                        state.color(&theme.colors)
                    }),
                ),
        )
}
