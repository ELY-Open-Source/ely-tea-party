//! The tea party: every session at one table, each character with what she does and says.

use std::time::Instant;

use ely_gpui_component::{
    buttons::{ButtonVariant, IconButton},
    primitives::{Icon, IconName},
    shell::{ColumnShell, ShellColumn},
    theme::{ActiveTheme, ControlSize, IconSize, Radius, TextSize},
};
use gpui::{
    AnyElement, App, Context, Entity, Focusable, FontWeight, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, rems,
};

use super::{TITLE, Workspace};
use crate::{
    agents::{self, Status, quips::Moment},
    sessions::{self, Session},
};

impl Workspace {
    /// Opens the tea party and gives it the keys, or goes back to the shown session.
    pub(super) fn toggle_party(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.party = !self.party;
        if self.party {
            window.focus(&self.party_focus, cx);
        } else if let Some(session) = self.sessions.get(self.active) {
            window.focus(&session.terminal.focus_handle(cx), cx);
        }
        log::info!(
            "app: the tea party {}",
            if self.party { "opens" } else { "closes" }
        );
        cx.notify();
    }

    /// The window's content while the party is open: one column, a card per session.
    pub(super) fn party(&self, cx: &mut Context<Self>) -> ColumnShell {
        let workspace = cx.entity();
        let now = Instant::now();
        let home = dirs::home_dir();
        let count = |status: Status| {
            let agents = self
                .sessions
                .iter()
                .filter_map(|session| session.agent.as_ref());
            agents.filter(|agent| agent.status == status).count()
        };
        let title = format!(
            "Tea Party · {} 位在忙 · {} 位等你 · {} 位休息",
            count(Status::Working),
            count(Status::NeedsInput),
            count(Status::Idle)
        );
        let back = {
            let workspace = workspace.clone();
            IconButton::new("close-party", IconName::LayoutGrid)
                .variant(ButtonVariant::Ghost)
                .size(ControlSize::Sm)
                .tooltip("Back to the session")
                .on_click(move |_, window, cx| {
                    workspace.update(cx, |this, cx| this.toggle_party(window, cx))
                })
        };
        let cards: Vec<AnyElement> = self
            .sessions
            .iter()
            .enumerate()
            .map(|(index, session)| {
                card(
                    &workspace,
                    index,
                    session,
                    index == self.active,
                    (home.as_deref(), now),
                    cx,
                )
            })
            .collect();
        let table = div()
            .id("party")
            .size_full()
            .overflow_y_scroll()
            .track_focus(&self.party_focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    this.toggle_party(window, cx);
                }
            }))
            .p_6()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .justify_center()
                    .gap_4()
                    .children(cards),
            );
        ColumnShell::new("party").glass().column(
            ShellColumn::new("party")
                .leading(back)
                .title(title)
                .child(table),
        )
    }
}

/// The sessions column's button that opens the party.
pub(super) fn party_button(workspace: Entity<Workspace>) -> IconButton {
    IconButton::new("open-party", IconName::LayoutGrid)
        .variant(ButtonVariant::Ghost)
        .size(ControlSize::Sm)
        .tooltip("Tea party")
        .on_click(move |_, window, cx| {
            workspace.update(cx, |this, cx| this.toggle_party(window, cx))
        })
}

/// Session `index` at the table: her bubble, her character, her doing; a click goes to her.
fn card(
    workspace: &Entity<Workspace>,
    index: usize,
    session: &Session,
    shown: bool,
    (home, now): (Option<&std::path::Path>, Instant),
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let quiet = |text: String, color| {
        div()
            .max_w_full()
            .truncate()
            .text_size(theme.text_size(TextSize::Sm))
            .text_color(color)
            .child(text)
    };
    let folder = sessions::brief(&session.folder, home);
    let (picture, name, lines, waits, bubble) = match &session.agent {
        Some(agent) => {
            let pose = agent.pose(now, session.id);
            let since = agents::ago(now.saturating_duration_since(agent.since));
            let mut lines = vec![quiet(
                format!("{} · {since}", agent.kind.label(pose)),
                colors.fg_muted,
            )];
            lines.extend(
                agent
                    .news(pose)
                    .map(|news| quiet(sessions::one_line(&news), colors.fg_subtle)),
            );
            if agent.turn.tools > 0 {
                let edited = agent.turn.edited.len();
                let stats = format!("这轮 {} 次工具 · 改了 {edited} 个文件", agent.turn.tools);
                lines.push(quiet(stats, colors.fg_subtle));
            }
            let picture =
                sessions::avatar(agent, pose, rems(8.), (session, now), cx).into_any_element();
            let bubble = agent.bubble(now);
            (
                picture,
                session.called(agent),
                lines,
                agent.status == Status::NeedsInput,
                bubble,
            )
        }
        None => {
            let picture = div()
                .size(rems(8.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(theme.radius(Radius::Lg))
                .bg(colors.sunken)
                .child(
                    Icon::new(IconName::Terminal)
                        .size(IconSize::Lg)
                        .color(colors.fg_muted),
                )
                .into_any_element();
            let name = if session.title.is_empty() {
                TITLE.to_string()
            } else {
                sessions::one_line(&session.title)
            };
            (picture, name, Vec::new(), session.bell, None)
        }
    };
    // Room for a bubble on every card, so the table does not jump when one speaks.
    let speech = div()
        .h(rems(2.))
        .flex()
        .items_end()
        .children(bubble.map(|(moment, line)| {
            let color = if moment == Moment::Failed {
                colors.danger
            } else {
                colors.fg
            };
            div()
                .px_2()
                .py_0p5()
                .rounded(theme.radius(Radius::Md))
                .bg(colors.overlay)
                .border_1()
                .border_color(colors.border)
                .text_size(theme.text_size(TextSize::Sm))
                .text_color(color)
                .child(format!("「{line}」"))
        }));
    let border = if waits {
        colors.warning
    } else if shown {
        colors.focus
    } else {
        colors.border
    };
    let workspace = workspace.clone();
    div()
        .id(("party-card", session.id as usize))
        .w(rems(15.))
        .p_4()
        .flex()
        .flex_col()
        .items_center()
        .gap_1()
        .rounded(theme.radius(Radius::Lg))
        .bg(colors.surface.opacity(0.6))
        .border_2()
        .border_color(border)
        .cursor_pointer()
        .hover(|style| style.bg(colors.hover))
        .on_click(move |_, window, cx| {
            workspace.update(cx, |this, cx| this.select(index, window, cx))
        })
        .child(speech)
        .child(picture)
        .child(
            div()
                .pt_1()
                .text_size(theme.text_size(TextSize::Base))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors.fg)
                .child(name),
        )
        .children(lines)
        .child(quiet(folder, colors.fg_subtle))
        .when(waits, |card| {
            card.child(quiet("举手：等你回复".into(), colors.warning))
        })
        .into_any_element()
}
