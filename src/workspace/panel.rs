//! The right column: Git, Files, or the shown session's Activity, one at a time.

use std::time::{Duration, Instant};

use ely_gpui_component::{
    buttons::SegmentedControl,
    shell::ShellColumn,
    theme::{ActiveTheme, ControlSize, TextSize},
    typography::Caption,
};
use gpui::{
    AnyElement, App, Context, Div, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, StyleRefinement, Styled, div, prelude::FluentBuilder,
};

use super::{Side, Workspace, header::show_tab};
use crate::{
    agents::{
        self,
        timeline::{Entry, Step},
    },
    sessions::{self, Session},
    settings::PanelTab,
};

impl Workspace {
    /// The right column, ending in `end`, the title bar's last controls.
    pub(super) fn panel(&self, tab: PanelTab, end: Div, cx: &mut Context<Self>) -> ShellColumn {
        let chosen = match tab {
            PanelTab::Git => "git",
            PanelTab::Files => "files",
            PanelTab::Activity => "activity",
        };
        let switch = SegmentedControl::new("panel-tab", chosen)
            .segment("git", "Git", None)
            .segment("files", "Files", None)
            .segment("activity", "Activity", None)
            .size(ControlSize::Sm)
            .on_change(|value, _, cx| show_tab(value, cx));
        let session = self.sessions.get(self.active);
        let quiet = tab != PanelTab::Activity && session.is_some_and(|session| !session.reports);
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
        let fill = || StyleRefinement::default().size_full();
        self.side_column(Side::Panel, cx)
            .action(end)
            // Below the title bar, clear of Windows' caption buttons.
            .child(div().flex().justify_center().px_2().pt_2().child(switch))
            .children(quiet.then(caption))
            .child(body(tab == PanelTab::Git).child(self.git.clone().cached(fill())))
            .child(body(tab == PanelTab::Files).child(self.files.clone().cached(fill())))
            .children((tab == PanelTab::Activity).then(|| body(true).child(activity(session, cx))))
            .children(self.usage(cx))
    }
}

/// `took` short: `0.4s`, `12s`, then as rows count time.
fn short(took: Duration) -> String {
    if took < Duration::from_secs(10) {
        format!("{:.1}s", took.as_secs_f32())
    } else {
        agents::ago(took)
    }
}

/// The shown session's agent's steps, newest first.
fn activity(session: Option<&Session>, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let Some(agent) = session.and_then(|session| session.agent.as_ref()) else {
        return Caption::new("No agent runs in this session.")
            .px_1()
            .pt_2()
            .into_any_element();
    };
    if agent.log.is_empty() {
        let hint = "Nothing yet: steps show once the agent's hooks report.";
        return Caption::new(hint).px_1().pt_2().into_any_element();
    }
    let now = Instant::now();
    let row = |entry: &Entry| {
        let (text, color, took) = match &entry.step {
            Step::Prompt => ("—— 新的一轮 ——".to_string(), colors.fg_muted, None),
            Step::Tool {
                doing,
                name,
                detail,
                took,
                failed,
            } => {
                let what = detail.as_deref().unwrap_or(name);
                let text = sessions::one_line(&format!("{} {what}", doing.verb()));
                let color = if *failed { colors.danger } else { colors.fg };
                (text, color, Some(took.map_or("…".into(), short)))
            }
            Step::Asked(detail) => {
                let asked = detail.as_deref().unwrap_or("需要你回复");
                (
                    sessions::one_line(&format!("等你：{asked}")),
                    colors.warning,
                    None,
                )
            }
            Step::Ended { failed, took } => {
                let text = if *failed {
                    "出错结束"
                } else {
                    "这轮结束"
                };
                let color = if *failed {
                    colors.danger
                } else {
                    colors.success
                };
                (text.to_string(), color, took.map(short))
            }
        };
        div()
            .flex()
            .gap_2()
            .py_0p5()
            .text_size(theme.text_size(TextSize::Sm))
            .child(
                div()
                    .w_10()
                    .flex_none()
                    .text_color(colors.fg_subtle)
                    .child(agents::ago(now.saturating_duration_since(entry.at))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(color)
                    .child(text),
            )
            .children(took.map(|took| div().flex_none().text_color(colors.fg_subtle).child(took)))
    };
    div()
        .id("activity")
        .size_full()
        .overflow_y_scroll()
        .px_1()
        .children(agent.log.iter().rev().map(row))
        .into_any_element()
}
